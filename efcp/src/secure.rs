use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use bytes::BufMut;
use channel::{derive_packet, BasePacket, Channel, Packet};
use disco::{StatelessTransportState, TAG_LEN};
use std::io::{Error, ErrorKind, Result};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub struct DiscoPacket<P: BasePacket>(P);

impl<P: BasePacket> BasePacket for DiscoPacket<P> {
    fn new(payload_len: usize) -> Self {
        let mut packet = P::new(payload_len + 8 + TAG_LEN);
        packet.put_u64_be(0);
        packet.put_slice(&[0u8; TAG_LEN][..]);
        Self(packet)
    }

    fn check(&self) -> Result<()> {
        if self.0.payload().len() < 8 {
            return Err(Error::new(ErrorKind::Other, "invalid disco packet"));
        }
        Ok(())
    }

    fn payload(&self) -> &[u8] {
        &self.0.payload()[(8 + TAG_LEN)..]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.0.payload_mut()[(8 + TAG_LEN)..]
    }

    fn debug(&self, ds: &mut std::fmt::DebugStruct) {
        self.0.debug(ds);
        ds.field("nonce", &self.nonce());
    }
}

derive_packet!(DiscoPacket);

impl<P: BasePacket> DiscoPacket<P> {
    pub fn nonce(&self) -> u64 {
        BigEndian::read_u64(&self.0.payload()[..8])
    }

    pub fn set_nonce(&mut self, nonce: u64) {
        BigEndian::write_u64(&mut self.0.payload_mut()[..8], nonce)
    }

    pub fn tag(&self) -> [u8; TAG_LEN] {
        let mut tag = [0u8; TAG_LEN];
        tag.copy_from_slice(&self.0.payload()[8..(8 + TAG_LEN)]);
        tag
    }

    pub fn set_tag(&mut self, tag: [u8; TAG_LEN]) {
        self.0.payload_mut()[8..(8 + TAG_LEN)].copy_from_slice(&tag[..]);
    }
}

pub struct DiscoChannel<C> {
    state: StatelessTransportState,
    channel: C,
    nonce: AtomicU64,
}

impl<C: Channel> DiscoChannel<C> {
    pub fn new(channel: C, state: StatelessTransportState) -> Self {
        Self {
            state,
            channel,
            nonce: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl<C: Channel> Channel for DiscoChannel<C> {
    type Packet = DiscoPacket<C::Packet>;

    async fn send(&self, mut packet: Self::Packet) -> Result<()> {
        let nonce = self.nonce.fetch_add(1, Ordering::SeqCst);
        packet.set_nonce(nonce);
        let tag = self.state.write_message(nonce, packet.payload_mut());
        packet.set_tag(tag);
        self.channel.send(packet.into_packet()).await
    }

    async fn recv(&self) -> Result<Self::Packet> {
        let mut packet = DiscoPacket::parse(self.channel.recv().await?)?;
        let nonce = packet.nonce();
        let tag = packet.tag();
        self.state
            .read_message(nonce, packet.payload_mut(), tag)
            .map_err(|err| Error::new(ErrorKind::Other, format!("{:?}", err)))?;
        Ok(packet)
    }
}

impl<C> core::ops::Deref for DiscoChannel<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use disco::SessionBuilder;
    use dtp::DtpSocket;

    async fn disco_channel() {
        let mut s1 = SessionBuilder::new("NN").build_initiator();
        let mut s2 = SessionBuilder::new("NN").build_responder();
        let m1 = s1.write_message(&[]);
        s2.read_message(&m1).unwrap();
        let m2 = s2.write_message(&[]);
        s1.read_message(&m2).unwrap();
        let t1 = s1.into_stateless_transport_mode();
        let t2 = s2.into_stateless_transport_mode();
        let d1 = DtpSocket::bind("/ip4/127.0.0.1")
            .await
            .unwrap();
        let d2 = DtpSocket::bind("/ip4/127.0.0.1")
            .await
            .unwrap();
        let c1 = d1.outgoing(d2.local_addr().unwrap(), 0).unwrap();
        let c2 = d2.outgoing(d1.local_addr().unwrap(), 0).unwrap();
        let c1 = DiscoChannel::new(c1, t1);
        let c2 = DiscoChannel::new(c2, t2);
        println!("setup finished");
        c1.send("ping".into()).await.unwrap();
        let m1 = c2.recv().await.unwrap();
        assert_eq!(m1.payload(), b"ping");
        c2.send("pong".into()).await.unwrap();
        let m2 = c1.recv().await.unwrap();
        assert_eq!(m2.payload(), b"pong");
    }

    #[test]
    fn test_disco_channel() {
        task::block_on(disco_channel());
    }
}
