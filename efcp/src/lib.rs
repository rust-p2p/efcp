//! # Error and flow control protocol
//! Initially a derivation of the delta-t transport protocol for use in the
//! RINA network architecture. It tries to stay true to the concepts but
//! adapted to the UDP/IP infrastructure. In addition encryption based on
//! the noise and disco specifications are mandated, encryption being an
//! optional feature in the original EFCP incarnation.
//!
//! ## Security considerations
//! Padding: All messages must be padded to equal length to prevent information
//!   leakage.
//! Termination: Session termination must be made explicit to prevent truncation
//!   attacks.
//! Nonces: Every message has a clear text nonce. Nonces are not allowed to wrap
//!   since that would open the possibility of messages being replayed. For this
//!   reason the nonce must be  minimum 64 bits in length. The assumption is
//!   made that with todays technology a nonce of 64-bits will never wrap.
//!
//! ## Handshake
//! The default handshake is based on the `XK1sig` pattern from the noise
//! signature extension spec. The client identity is useful for performing
//! access control operations.
//!
//! XK1sig
//!   <- s
//!   ...
//!   -> e
//!   <- e ee sig
//!   -> s sig
//!
//!
//! ## Observed address and NAT traversal
//! In P2P networks knowledge of the external address port tuple is neccessary
//! to advertise availability on a DHT. For this reason during a handshake
//! message the first response from a responder must include the observed
//! address and port.
//!
//! ## Generic protocol negotiation
//! Based on the libp2p connection spec, a protocol identifier is sent by the
//! initiator to request a protocol. The responder can accept by echoing the
//! protocol identifier or deny with a N/A message. On failure the negotiation
//! can be restarted. This process is (by default) started in parallel with the
//! handshake to reduce latency.
//!
//! ### DTCP parameter negotiation
//! The congestion and flow control elements of the protocol have different
//! parameters to make it tunable to a specific application. These are
//! negotiated using the generic protocol negotiation mechanism.
//!
//! ### Application protocol negotiation
//! The application protocol has a unique protocol identifier, which is used
//! to negotiate the application protocol using the generic protocol negotiation
//! mechanism.
mod error;
mod negotiation;
mod packet;
mod secure;

use crate::error::HandshakeError;
use crate::negotiation::{Negotiation, Protocol, Protocols};
use crate::packet::HandshakePacket;
use crate::secure::{DiscoChannel, DiscoPacket};
use async_std::prelude::*;
use async_trait::async_trait;
use channel::{BasePacket, Channel};
use disco::ed25519::{Keypair, PublicKey};
use disco::SessionBuilder;
use dtcp::{DtcpBuilder, DtcpChannel, DtcpPacket};
use dtp::{DtpChannel, DtpPacket, DtpSocket};
use std::io::Error;
use std::net::SocketAddr;

pub struct Dial {
    pub peer_addr: SocketAddr,
    pub channel: u8,
    pub remote_public: PublicKey,
    pub protocols: Protocols,
}

pub struct EfcpSocket {
    dtp: DtpSocket,
    dtcp: DtcpBuilder,
    session: SessionBuilder,
    protocols: Protocols,
}

impl EfcpSocket {
    pub async fn bind(
        addr: SocketAddr,
        identity: Keypair,
        protocols: Protocols,
    ) -> Result<Self, Error> {
        let dtp = DtpSocket::bind(addr).await?;
        let dtcp = DtcpBuilder::new();
        let session = SessionBuilder::new("XK1sig").secret(identity);
        Ok(Self {
            dtp,
            dtcp,
            session,
            protocols,
        })
    }

    pub async fn incoming(&self) -> Option<Result<EfcpChannel, HandshakeError>> {
        match self.dtp.incoming().next().await {
            Some(Ok(dtp)) => {
                let peer_addr = Some(dtp.peer_addr().clone());
                let efcp = EfcpChannel::responder(
                    dtp,
                    self.dtcp.clone(),
                    self.session.clone(),
                    self.protocols,
                    peer_addr,
                )
                .await;
                Some(efcp)
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }

    pub async fn dial(&self, dial: &Dial) -> Result<EfcpChannel, HandshakeError> {
        // TODO channel allocation
        let channel = self.dtp.outgoing(dial.peer_addr, dial.channel)?;
        self.session.remote_public(dial.remote_public);
        EfcpChannel::initiator(
            channel,
            self.dtcp.clone(),
            self.session.clone(),
            self.protocols,
        )
        .await
    }
}

pub struct EfcpChannel {
    channel: DtcpChannel<DiscoChannel<DtpChannel>>,
    remote: PublicKey,
    protocol: Protocol,
    external_addr: Option<SocketAddr>,
}

impl EfcpChannel {
    async fn initiator(
        dtp: DtpChannel,
        dtcp: DtcpBuilder,
        session: SessionBuilder,
        protocols: Protocols,
    ) -> Result<Self, HandshakeError> {
        let channel = dtcp.build_channel(dtp);
        let mut session = session.build_initiator();
        let mut negotiate = Negotiation::new(protocols);
        let mut external_addr = None;
        let mut next_neg = Some(negotiate.initiate());
        loop {
            let msg = HandshakePacket::new(next_neg, None);
            let ct = session.write_message(&msg.to_bytes());
            let dtcp = DtcpPacket::from(&ct[..]);
            channel.send(dtcp).await?;

            let dtcp = channel.recv().await?;
            let bytes = session.read_message(dtcp.payload())?;
            let msg = HandshakePacket::from_bytes(&bytes)?;
            if let Some(addr) = msg.external_addr() {
                external_addr = Some(addr);
            }
            if let Some(msg) = msg.negotiate() {
                next_neg = negotiate.message(msg)?;
            }

            if session.is_handshake_finished() && negotiate.is_finished() {
                break;
            }
        }

        let remote = *session
            .get_remote_static()
            .expect("XK1sig handshake; qed")
            .ed25519();
        let session = session.into_stateless_transport_mode();

        let protocol = if let Some(protocol) = negotiate.into_protocol() {
            protocol
        } else {
            return Err(HandshakeError::Negotiation);
        };

        if external_addr.is_none() {
            return Err(HandshakeError::ExternalAddr);
        }

        let channel = channel.unwrap();
        let channel = DiscoChannel::new(channel, session);
        let channel = dtcp.build_channel(channel);

        Ok(Self {
            channel,
            remote,
            protocol,
            external_addr,
        })
    }

    async fn responder(
        dtp: DtpChannel,
        dtcp: DtcpBuilder,
        session: SessionBuilder,
        protocols: Protocols,
        mut external_addr: Option<SocketAddr>,
    ) -> Result<Self, HandshakeError> {
        let channel = dtcp.build_channel(dtp);
        let session = session.build_responder();
        let negotiate = Negotiation::new(protocols);

        loop {
            let dtcp = channel.recv().await?;
            let bytes = session.read_message(dtcp.payload())?;
            let msg = HandshakePacket::from_bytes(&bytes[..])?;
            let next_neg = if let Some(msg) = msg.negotiate() {
                negotiate.message(msg)?
            } else {
                None
            };

            let msg = HandshakePacket::new(next_neg, external_addr.take());
            let ct = session.write_message(&msg.to_bytes());
            let dtcp = DtcpPacket::from(&ct[..]);
            channel.send(dtcp).await?;

            if session.is_handshake_finished() && negotiate.is_finished() {
                break;
            }
        }

        let remote = *session
            .get_remote_static()
            .expect("XK1sig handshake; qed")
            .ed25519();
        let session = session.into_stateless_transport_mode();

        let protocol = if let Some(protocol) = negotiate.into_protocol() {
            protocol
        } else {
            return Err(HandshakeError::Negotiation);
        };

        let channel = channel.unwrap();
        let channel = DiscoChannel::new(channel, session);
        let channel = dtcp.build_channel(channel);

        Ok(Self {
            channel,
            remote,
            protocol,
            external_addr,
        })
    }

    pub fn peer_id(&self) -> PublicKey {
        self.remote
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn external_addr(&self) -> Option<&SocketAddr> {
        self.external_addr.as_ref()
    }
}

#[async_trait]
impl Channel for EfcpChannel {
    type Packet = DtcpPacket<DiscoPacket<DtpPacket>>;

    async fn send(&self, packet: Self::Packet) -> Result<(), Error> {
        self.channel.send(packet).await
    }

    async fn recv(&self) -> Result<Self::Packet, Error> {
        self.channel.recv().await
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
