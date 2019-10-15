//! Defines the DTCP packet format.
use bytes::BufMut;
use byteorder::{BigEndian, ByteOrder};
use dtp::Packet;
use std::io::{Error, ErrorKind, Result};

/// Type of PDU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DtcpType {
    /// Data PDU.
    Transfer = 0,
    /// Control PDU.
    Control = 1,
}

/// DTCP Header:
///   type: u8
///   sequence_number: u16
#[derive(Clone)]
pub struct DtcpPacket(Packet);

impl DtcpPacket {
    /// Creates a new dtcp packet that fits the payload.
    pub fn new(payload_len: usize) -> Self {
        let mut packet = Packet::new(payload_len + 3);
        packet.put_u8(0);
        packet.put_u16_be(0);
        Self(packet)
    }

    /// Parses a packet.
    pub fn parse(packet: Packet) -> Result<Self> {
        if packet.payload().len() < 3 {
            return Err(Self::invalid());
        }

        if packet.payload()[0] >= 2 {
            return Err(Self::invalid());
        }

        Ok(Self(packet))
    }

    fn invalid() -> Error {
        Error::new(ErrorKind::Other, "invalid dtcp packet")
    }

    fn raw_type(&self) -> u8 {
        self.0.payload()[0]
    }
    
    pub(crate) fn ty(&self) -> DtcpType {
        match self.raw_type() {
            0 => DtcpType::Transfer,
            1 => DtcpType::Control,
            _ => unreachable!(),
        }
    }

    pub(crate) fn set_ty(&mut self, ty: DtcpType) {
        self.0.payload_mut()[0] = ty as u8;
    }
    
    pub(crate) fn seq_num(&self) -> u16 {
        BigEndian::read_u16(&self.0.payload()[1..3])
    }

    pub(crate) fn set_seq_num(&mut self, seq_num: u16) {
        BigEndian::write_u16(&mut self.0.payload_mut()[1..3], seq_num)
    }

    /// Returns the payload of a packet.
    pub fn payload(&self) -> &[u8] {
        &self.0.payload()[3..]
    }

    /// Returns the mutable payload of a packet.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.0.payload_mut()[3..]
    }

    /// Returns a packet.
    pub fn into_packet(self) -> Packet {
        self.0
    }
}

impl BufMut for DtcpPacket {
    fn remaining_mut(&self) -> usize {
        self.0.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, by: usize) {
        self.0.advance_mut(by)
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        self.0.bytes_mut()
    }
}

impl From<&[u8]> for DtcpPacket {
    fn from(payload: &[u8]) -> Self {
        let mut packet = DtcpPacket::new(payload.len());
        packet.put(payload);
        packet
    }
}

impl From<&str> for DtcpPacket {
    fn from(payload: &str) -> Self {
        Self::from(payload.as_bytes())
    }
}

impl std::fmt::Debug for DtcpPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("DtcpPacket")
            .field("ecn", &self.0.ecn())
            .field("channel", &self.0.channel())
            .field("type", &self.ty())
            .field("seq_num", &self.seq_num())
            .field("payload", &self.payload().len())
            .finish()
    }
}
