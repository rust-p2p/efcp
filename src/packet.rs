//! Packet format adapted from the spec.
//!
//! Dropped fields already present in or not applicable to UDP/IP:
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
use bytes::{BytesMut, BufMut};
use byteorder::{ByteOrder, BigEndian};
use std::io::{Error, ErrorKind, Result};

/// Packet header
///
/// ```text
/// 0       8       16      24      32
/// +-------+-------+-------+-------+
/// | ver   | type  | flags         |
/// +-------+-------+-------+-------+
/// | sequence number               |   
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

/// The length of sequence numbers needs to be roughly
///   2^n > (2MPL + R + A) * T
/// where
///   MPL: Maximum PDU lifetime
///   R: Maximum time for retries
///   A: Maximum time before an ack is sent
///   T: Data rate at which sequence numbers are incremented.
pub type SequenceNumber = u32;

/// Length of the IP header.
const IP_HEADER_LEN: usize = 20;
/// Length of the UDP header.
const UDP_HEADER_LEN: usize = 8;
/// Length of the packet header.
pub const HEADER_LEN: usize = 8;

/// Maximum length of the payload.
pub const MAX_PAYLOAD_LEN: usize = {
    std::u16::MAX as usize - IP_HEADER_LEN - UDP_HEADER_LEN - HEADER_LEN
};

const DEFAULT: [u8; 8] = [
    // Version, type and flags
    1, 0, 0, 0,
    // Default sequence number
    0, 0, 0, 0,
];

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
    pub fn sequence_number(&self) -> u32 {
        BigEndian::read_u32(&self.data[4..8])
    }
    
    /// Set sequence number.
    pub fn set_sequence_number(&mut self, sn: u32) {
        BigEndian::write_u32(&mut self.data[4..8], sn);
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
    pub fn data_run(&self) -> bool {
        self.data[2] & 0b1000_0000 > 0
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
            .field("data run", &self.data_run())
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
