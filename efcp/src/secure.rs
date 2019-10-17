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
        Self(packet)
    }

    fn check(&self) -> Result<()> {
        if self.0.payload().len() < 8 {
            return Err(Error::new(ErrorKind::Other, "invalid disco packet"));
        }
        Ok(())
    }

    fn payload(&self) -> &[u8] {
        &self.0.payload()[8..]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.0.payload_mut()[8..]
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
        BigEndian::write_u64(&self.0.payload_mut()[..8], nonce)
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
            nonce: 0,
        }
    }
}

#[async_trait]
impl<C: Channel> Channel for DiscoChannel<C> {
    type Packet = DiscoPacket<C::Packet>;

    async fn send(&self, mut packet: Self::Packet) -> Result<()> {
        let nonce = self.nonce.fetch_add(1, Ordering::SeqCst);
        packet.set_nonce(nonce);
        self.state.write_message(nonce, packet.payload_mut());
        self.channel.send(packet.into_packet()).await
    }

    async fn recv(&self) -> Result<Self::Packet> {
        let mut packet = DiscoPacket::parse(self.channel.recv().await?)?;
        self.state
            .read_message(packet.nonce(), packet.payload_mut())
            .map_err(|err| Error::new(ErrorKind::Other, format!("{:?}", err)))?;
        Ok(packet)
    }
}
