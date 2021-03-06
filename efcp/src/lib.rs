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
//! ```no_build
//! XK1sig
//!   <- s
//!   ...
//!   -> e
//!   <- e ee sig
//!   -> s sig
//! ```
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
#![deny(missing_docs)]
#![deny(warnings)]
mod error;
mod negotiation;
mod packet;
mod secure;

use crate::error::HandshakeError;
use crate::negotiation::{Negotiation, Protocol, Protocols};
use crate::packet::HandshakePacket;
use crate::secure::{DiscoChannel, DiscoPacket};
use addr::{Addr, ToAddr};
use async_std::prelude::*;
use async_trait::async_trait;
use channel::{BasePacket, Channel};
pub use disco::ed25519::{Keypair, PublicKey};
use disco::SessionBuilder;
use dtcp::{DtcpBuilder, DtcpChannel, DtcpPacket};
use dtp::{DtpChannel, DtpPacket, DtpSocket};
use std::io::Error;

/// The information required to dial a peer.
pub struct Dial {
    /// Peer's external address.
    pub peer_addr: Addr,
    /// DTP channel id.
    pub channel: u8,
    /// Peer's public key.
    pub remote_public: PublicKey,
    /// Acceptable application protocols.
    pub protocols: Protocols,
}

/// An EFCP socket.
///
/// Wraps a `DtpSocket` with a secure reliable transport.
///
/// ## Examples
///
/// ```no_run
/// # use async_std::prelude::*;
/// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
/// #
/// use channel::{BasePacket, Channel};
/// use efcp::{EfcpSocket, Keypair};
/// use rand::rngs::OsRng;
///
/// let identity = Keypair::generate(&mut OsRng);
/// let socket = EfcpSocket::bind(
///     "/ip4/0.0.0.0",
///     identity,
///     &["/ping/1.0"],
/// ).await?;
/// while let Some(channel) = socket.incoming().await {
///     let channel = channel?;
///     if channel.recv().await?.payload() == b"ping" {
///         channel.send("pong".into()).await?;
///     }
/// }
/// #
/// # Ok(()) }) }
/// ```
/// ```no_run
/// # use async_std::prelude::*;
/// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
/// # let remote_public = Keypair::generate(&mut OsRng).public;
/// #
/// use channel::{BasePacket, Channel};
/// use efcp::{Dial, EfcpSocket, Keypair};
/// use rand::rngs::OsRng;
///
/// let identity = Keypair::generate(&mut OsRng);
/// let socket = EfcpSocket::bind(
///     "/ip4/0.0.0.0",
///     identity,
///     &["/ping/1.0"],
/// ).await?;
/// let dial = Dial {
///     peer_addr: "/ip4/127.0.0.1/udp/8000".parse()?,
///     channel: 0,
///     remote_public,
///     protocols: &["/ping/1.0"],
/// };
/// let channel = socket.dial(&dial).await?;
/// channel.send("ping".into());
/// channel.recv().await?;
/// #
/// # Ok(()) }) }
/// ```
pub struct EfcpSocket {
    dtp: DtpSocket,
    identity: Keypair,
    protocols: Protocols,
}

impl EfcpSocket {
    /// Creates a new `EfcpSocket`.
    pub async fn bind<T: ToAddr>(
        addr: T,
        identity: Keypair,
        protocols: Protocols,
    ) -> Result<Self, Error> {
        let dtp = DtpSocket::bind(addr).await?;
        Ok(Self {
            dtp,
            identity,
            protocols,
        })
    }

    /// Returns a stream of incoming EFCP connections.
    pub async fn incoming(&self) -> Option<Result<EfcpChannel, HandshakeError>> {
        match self.dtp.incoming().next().await {
            Some(Ok(channel)) => {
                let peer_addr = channel.peer_addr().clone();
                let efcp =
                    EfcpChannel::responder(channel, &self.identity, self.protocols, peer_addr)
                        .await;
                Some(efcp)
            }
            Some(Err(err)) => Some(Err(err.into())),
            None => None,
        }
    }

    /// Dials a peer.
    pub async fn dial(&self, dial: &Dial) -> Result<EfcpChannel, HandshakeError> {
        let channel = self.dtp.outgoing(dial.peer_addr, dial.channel)?;
        EfcpChannel::initiator(channel, &self.identity, self.protocols, dial.remote_public).await
    }

    /// Returns the local address that this socket is bound to.
    pub fn local_addr(&self) -> Result<Addr, Error> {
        self.dtp.local_addr()
    }

    /// Returns the identity associated with this socket.
    pub fn identity(&self) -> PublicKey {
        self.identity.public
    }

    /// Returns the list of protocol supported on this socket.
    pub fn protocols(&self) -> Protocols {
        self.protocols
    }
}

/// A EFCP channel between a local and a remote socket.
pub struct EfcpChannel {
    channel: DtcpChannel<DiscoChannel<DtpChannel>>,
    remote: PublicKey,
    protocol: Protocol,
    external_addr: Option<Addr>,
}

impl EfcpChannel {
    async fn initiator(
        channel: DtpChannel,
        identity: &Keypair,
        protocols: Protocols,
        remote_public: PublicKey,
    ) -> Result<Self, HandshakeError> {
        let dtcp = DtcpBuilder::new();
        let channel = dtcp.build_channel(channel);
        let mut session = SessionBuilder::new("XK1sig")
            .secret(identity)
            .remote_public(remote_public)
            .build_initiator();
        let mut negotiate = Negotiation::new(protocols);
        let mut external_addr = None;
        let mut next_neg = Some(negotiate.initiate());

        loop {
            let msg = HandshakePacket::new(next_neg.take(), None);
            let ct = session.write_message(&msg.to_bytes()?);
            channel.send(ct[..].into()).await?;

            if session.is_handshake_finished() {
                break;
            }

            let packet = channel.recv().await?;
            let pt = session.read_message(packet.payload())?;
            let mut msg = HandshakePacket::from_bytes(&pt)?;
            if let Some(addr) = msg.external_addr() {
                external_addr = Some(addr);
            }
            next_neg = msg
                .negotiate()
                .as_ref()
                .map(|msg| negotiate.message(msg))
                .unwrap_or(Ok(None))?;
        }

        let remote = *session
            .get_remote_static()
            .expect("XK1sig handshake; qed")
            .ed25519();
        let session = session.into_stateless_transport_mode();

        let channel = channel.unwrap();
        let channel = DiscoChannel::new(channel, session);
        let channel = dtcp.build_channel(channel);

        if external_addr.is_none() {
            return Err(HandshakeError::ExternalAddr);
        }

        while !negotiate.is_finished() {
            let packet = channel.recv().await?;
            let mut msg = HandshakePacket::from_bytes(packet.payload())?;
            next_neg = msg
                .negotiate()
                .as_ref()
                .map(|msg| negotiate.message(msg))
                .unwrap_or(Ok(None))?;

            if next_neg.is_none() {
                break;
            }

            let msg = HandshakePacket::new(next_neg.take(), None).to_bytes()?;
            channel.send(msg[..].into()).await?;
        }
        let protocol = negotiate
            .into_protocol()
            .map(|p| Ok(p))
            .unwrap_or(Err(HandshakeError::Negotiation))?;

        Ok(Self {
            channel,
            remote,
            protocol,
            external_addr,
        })
    }

    async fn responder(
        channel: DtpChannel,
        identity: &Keypair,
        protocols: Protocols,
        remote_addr: Addr,
    ) -> Result<Self, HandshakeError> {
        let dtcp = DtcpBuilder::new();
        let channel = dtcp.build_channel(channel);
        let mut session = SessionBuilder::new("XK1sig")
            .secret(identity)
            .build_responder();
        let mut negotiate = Negotiation::new(protocols);
        let mut external_addr = Some(remote_addr);
        let mut next_neg;

        loop {
            let dtcp = channel.recv().await?;
            let bytes = session.read_message(dtcp.payload())?;
            let mut msg = HandshakePacket::from_bytes(&bytes[..])?;
            next_neg = msg
                .negotiate()
                .as_ref()
                .map(|msg| negotiate.message(msg))
                .unwrap_or(Ok(None))?;

            if session.is_handshake_finished() {
                break;
            }

            let msg = HandshakePacket::new(next_neg, external_addr.take());
            let ct = session.write_message(&msg.to_bytes()?);
            let dtcp = DtcpPacket::from(&ct[..]);
            channel.send(dtcp).await?;
        }

        let remote = *session
            .get_remote_static()
            .expect("XK1sig handshake; qed")
            .ed25519();
        let session = session.into_stateless_transport_mode();

        let channel = channel.unwrap();
        let channel = DiscoChannel::new(channel, session);
        let channel = dtcp.build_channel(channel);

        while let Some(msg) = next_neg.take() {
            let msg = HandshakePacket::new(Some(msg), external_addr.take()).to_bytes()?;
            channel.send(msg[..].into()).await?;

            if negotiate.is_finished() {
                break;
            }

            let packet = channel.recv().await?;
            let mut msg = HandshakePacket::from_bytes(packet.payload())?;
            next_neg = msg
                .negotiate()
                .as_ref()
                .map(|msg| negotiate.message(msg))
                .unwrap_or(Ok(None))?;
        }
        let protocol = negotiate
            .into_protocol()
            .map(|p| Ok(p))
            .unwrap_or(Err(HandshakeError::Negotiation))?;

        Ok(Self {
            channel,
            remote,
            protocol,
            external_addr,
        })
    }

    /// Returns the local address that this bound to.
    pub fn local_addr(&self) -> Result<Addr, Error> {
        self.channel.local_addr()
    }

    /// Returns the remote address that this channel is connected to.
    pub fn peer_addr(&self) -> &Addr {
        self.channel.peer_addr()
    }

    /// Returns the external address that the remote observed.
    pub fn external_addr(&self) -> Option<&Addr> {
        self.external_addr.as_ref()
    }

    /// Returns the channel id.
    pub fn channel(&self) -> u8 {
        self.channel.channel()
    }

    /// Returns the public key of the peer that this channel is connected to.
    pub fn peer_identity(&self) -> PublicKey {
        self.remote
    }

    /// Returns the protocol that this channel has negotiated.
    pub fn protocol(&self) -> Protocol {
        self.protocol
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
    use super::*;
    use async_std::task;
    use futures::join;
    use rand::rngs::OsRng;

    async fn efcp() -> Result<(), HandshakeError> {
        let addr = "/ip4/127.0.0.1";
        let protocols = &["/ping/1.0"];

        let identity1 = Keypair::generate(&mut OsRng);
        let socket1 = EfcpSocket::bind(addr, identity1, &["/ping/1.0"]).await?;

        let identity2 = Keypair::generate(&mut OsRng);
        let socket2 = EfcpSocket::bind(addr, identity2, &["/ping/1.0"]).await?;
        let external_addr = socket2.local_addr()?;

        /*let dial1 = Dial {
            peer_addr: socket2.local_addr()?,
            channel: 0,
            remote_public: identity2.public,
            protocols,
        };
        let channel1 = socket1.dial(dial1)?;*/

        let dial2 = Dial {
            peer_addr: socket1.local_addr()?,
            channel: 0,
            remote_public: socket1.identity(),
            protocols,
        };

        let channel1 = task::spawn(async move { socket1.incoming().await.unwrap().unwrap() });

        let channel2 = task::spawn(async move { socket2.dial(&dial2).await.unwrap() });

        let (channel1, channel2) = join!(channel1, channel2);

        assert_eq!(channel1.protocol(), "/ping/1.0");
        assert_eq!(channel2.protocol(), "/ping/1.0");
        assert_eq!(channel2.external_addr(), Some(&external_addr));

        channel2.send("ping".into()).await?;
        let msg = channel1.recv().await?;
        assert_eq!(msg.payload(), b"ping");

        channel1.send("pong".into()).await?;
        let msg = channel2.recv().await?;
        assert_eq!(msg.payload(), b"pong");

        Ok(())
    }

    #[test]
    fn test_efcp() {
        task::block_on(efcp()).unwrap();
    }
}
