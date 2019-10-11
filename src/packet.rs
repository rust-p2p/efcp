//! Packet format adapted from the spec.
//!
//! # Fields present in or not applicable to UDP/IP
//!
//! ```text
//! +---------------------+---------------------+
//! | EFCP field          | UDP/IP field        |
//! +=====================+=====================+
//! | Destination address | Destination address |
//! +---------------------+---------------------+
//! | Source address      | Source address      |
//! +---------------------+---------------------+
//! | QoS identifier      | N/A                 |
//! +---------------------+---------------------+
//! | Destination CEP-ID  | Destination Port    |
//! +---------------------+---------------------+
//! | Source CEP-ID       | Source Port         |
//! +---------------------+---------------------+
//! | Length              | Length              |
//! +---------------------+---------------------+
//! ```
use crate::constants::SequenceNumber;
use byteorder::{BigEndian, ByteOrder};
use bytes::{BufMut, BytesMut};
use std::io::{Error, ErrorKind, Result};

/// Packet header
///
/// ```text
/// 0       8       16      24      32
/// +-------+-------+-------+-------+
/// | ver   | type  | flags         |
/// +-------+-------+-------+-------+
/// | sequence number                  
/// +-------+-------+-------+-------+
///                                 |
/// +-------+-------+-------+-------+
/// ```
#[derive(Clone)]
pub struct Packet {
    data: BytesMut,
}

/// Type of PDU.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Type {
    /// DTP PDU type.
    Transfer = 0,
    /// DTCP PDU type.
    Control = 1,
}

/// Length of the IP header.
const IP_HEADER_LEN: usize = 20;
/// Length of the UDP header.
const UDP_HEADER_LEN: usize = 8;
/// Length of the packet header.
pub const HEADER_LEN: usize = 12;

/// Maximum length of the payload.
pub const MAX_PAYLOAD_LEN: usize =
    { std::u16::MAX as usize - IP_HEADER_LEN - UDP_HEADER_LEN - HEADER_LEN };

const DEFAULT: [u8; 12] = [
    // Version, type and flags
    1, 0, 0, 0, // Default sequence number
    0, 0, 0, 0, 0, 0, 0, 0,
];

const DRF_MASK: u8 = 0b1000_0000;

impl Packet {
    /// Parse a packet.
    pub fn parse(bytes: BytesMut) -> Result<Self> {
        let packet = Packet::new(bytes);

        if packet.version() != 1 {
            return Err(Error::new(ErrorKind::Other, "invalid packet version"));
        }

        if packet.raw_type() >= 2 {
            return Err(Error::new(ErrorKind::Other, "invalid packet type"));
        }

        Ok(packet)
    }

    /// Create a new packet.
    pub fn new(data: BytesMut) -> Self {
        Self { data }
    }

    /// Create a dtp packet.
    pub fn dtp(payload: &[u8]) -> Self {
        let mut bytes = BytesMut::with_capacity(HEADER_LEN + payload.len());
        bytes.put_slice(&DEFAULT);
        bytes.put_slice(payload);

        let mut packet = Packet::new(bytes);
        packet.set_type(Type::Transfer);
        packet
    }

    /// Version.
    pub fn version(&self) -> u8 {
        self.data[0]
    }

    fn raw_type(&self) -> u8 {
        self.data[1]
    }

    /// Get type.
    pub fn ty(&self) -> Type {
        match self.raw_type() {
            0 => Type::Transfer,
            1 => Type::Control,
            _ => unreachable!(),
        }
    }

    /// Set type.
    pub fn set_type(&mut self, ty: Type) {
        self.data[1] = ty as u8;
    }

    /// Get sequence number.
    pub fn sequence_number(&self) -> SequenceNumber {
        BigEndian::read_u64(&self.data[4..12])
    }

    /// Set sequence number.
    pub fn set_sequence_number(&mut self, sn: SequenceNumber) {
        BigEndian::write_u64(&mut self.data[4..12], sn);
    }

    /// Get the user data.
    pub fn payload(&self) -> &[u8] {
        &self.data[HEADER_LEN..]
    }

    /// Get owned user data.
    pub fn into_payload(mut self) -> BytesMut {
        self.data.split_to(HEADER_LEN);
        self.data
    }

    /// Data run flag.
    pub fn drf(&self) -> bool {
        self.data[2] & DRF_MASK > 0
    }

    /// Set data run flag.
    pub fn set_drf(&mut self, drf: bool) {
        if drf {
            self.data[2] |= DRF_MASK;
        } else {
            self.data[2] &= !DRF_MASK;
        }
    }

    /// Explicit congestion notification.
    pub fn ecn(&self) -> bool {
        self.data[2] & 0b0100_0000 > 0
    }

    /// Flow control information present.
    pub fn fci(&self) -> bool {
        self.data[2] & 0b0000_1000 > 0
    }

    /// Ack/Nack information present.
    pub fn acki(&self) -> bool {
        self.data[2] & 0b0000_0100 > 0
    }

    /// Selective ack/nack.
    pub fn sel_ack(&self) -> bool {
        self.data[2] & 0b0000_0010 > 0
    }

    /// Ack/Nack.
    pub fn ack(&self) -> bool {
        self.data[2] & 0b0000_0001 > 0
    }
}

impl Default for Packet {
    fn default() -> Self {
        Packet::new(BytesMut::from(&DEFAULT[..]))
    }
}

impl std::fmt::Debug for Packet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Packet")
            .field("version", &self.version())
            .field("type", &self.ty())
            .field("drf", &self.drf())
            .field("ecn", &self.ecn())
            .field("flow control information present", &self.fci())
            .field("ack/nack information present", &self.acki())
            .field("selective ack/nack", &self.sel_ack())
            .field("ack/nack", &self.ack())
            .field("sequence number", &self.sequence_number())
            .field("payload", &self.payload().len())
            .finish()
    }
}
