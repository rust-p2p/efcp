use bytes::{BufMut, BytesMut};

const MAX_PACKET_LEN: usize = std::u16::MAX as usize;
// const IP4_HEADER_LEN: usize = 20;
const IP6_HEADER_LEN: usize = 40;
const UDP_HEADER_LEN: usize = 8;
const MAX_HEADER_LEN: usize = IP6_HEADER_LEN + UDP_HEADER_LEN + 1;
/// The maximum length of a payload.
pub const MAX_PAYLOAD_LEN: usize = MAX_PACKET_LEN - MAX_HEADER_LEN;

/// A packet sendable via dtp.
#[derive(Clone, PartialEq, Eq)]
pub struct Packet {
    ecn: bool,
    bytes: BytesMut,
}

impl Packet {
    /// Creates a new packet that fits the payload.
    pub fn new(payload_len: usize) -> Self {
        debug_assert!(payload_len <= MAX_PAYLOAD_LEN);
        let mut bytes = BytesMut::with_capacity(payload_len + 1);
        bytes.put_u8(0);
        Self { ecn: false, bytes }
    }

    pub(crate) fn uninitialized() -> Self {
        let bytes = BytesMut::with_capacity(MAX_PACKET_LEN);
        Self { ecn: false, bytes }
    }

    pub(crate) unsafe fn set_len(&mut self, len: usize) {
        self.bytes.set_len(len);
    }

    /// Returns the explicit congestion notification bit.
    pub fn ecn(&self) -> bool {
        self.ecn
    }

    /// Sets an explicit congestion notification.
    pub fn set_ecn(&mut self, ecn: bool) {
        self.ecn = ecn;
    }

    /// Returns the channel of a packet.
    pub fn channel(&self) -> u8 {
        self.bytes[0]
    }

    pub(crate) fn set_channel(&mut self, channel: u8) {
        self.bytes[0] = channel;
    }

    /// Returns the payload of the packet.
    pub fn payload(&self) -> &[u8] {
        &self.bytes[1..]
    }

    /// Returns the mutable payload of the packet.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[1..]
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl BufMut for Packet {
    fn remaining_mut(&self) -> usize {
        self.bytes.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, by: usize) {
        self.bytes.advance_mut(by);
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        self.bytes.bytes_mut()
    }
}

impl From<&[u8]> for Packet {
    fn from(payload: &[u8]) -> Self {
        let mut packet = Packet::new(payload.len());
        packet.put(payload);
        packet
    }
}

impl From<&str> for Packet {
    fn from(payload: &str) -> Self {
        Self::from(payload.as_bytes())
    }
}

impl std::fmt::Debug for Packet {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Packet")
            .field("ecn", &self.ecn())
            .field("channel", &self.channel())
            .field("payload", &self.payload().len())
            .finish()
    }
}
