//! Defines the DTCP packet format.
use byteorder::{BigEndian, ByteOrder};
use bytes::BufMut;
use channel::{derive_packet, BasePacket};
use std::io::{Error, ErrorKind, Result};

/// Type of PDU.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DtcpType {
    /// Data PDU.
    Transfer {
        /// Data run flag indicates that this is the first packet of a run and
        /// that all previous packets have been acked.
        drf: bool,
    },
    /// Control PDU.
    Control,
}

/// DTCP Header:
///   type: u8
///   sequence_number: u16
#[derive(Clone)]
pub struct DtcpPacket<P>(P);

impl<P: BasePacket> BasePacket for DtcpPacket<P> {
    fn new(payload_len: usize) -> Self {
        let mut packet = P::new(payload_len + 3);
        packet.put_u8(0);
        packet.put_u16_be(0);
        Self(packet)
    }

    fn check(&self) -> Result<()> {
        if self.0.payload().len() < 3 {
            return Err(Error::new(ErrorKind::Other, "invalid dtcp packet"));
        }
        if self.raw_type() >= 2 {
            return Err(Error::new(ErrorKind::Other, "invalid dtcp packet type"));
        }
        Ok(())
    }

    fn payload(&self) -> &[u8] {
        &self.0.payload()[3..]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.0.payload_mut()[3..]
    }

    fn debug(&self, ds: &mut std::fmt::DebugStruct) {
        self.0.debug(ds);
        ds.field("type", &self.ty());
        ds.field("seq_num", &self.seq_num());
    }
}

derive_packet!(DtcpPacket);

impl<P: BasePacket> DtcpPacket<P> {
    fn raw_type(&self) -> u8 {
        self.0.payload()[0] >> 4
    }

    fn flag0(&self) -> bool {
        self.0.payload()[0] & 0b0001 > 0
    }

    pub(crate) fn ty(&self) -> DtcpType {
        match self.raw_type() {
            0 => DtcpType::Transfer { drf: self.flag0() },
            1 => DtcpType::Control,
            _ => unreachable!(),
        }
    }

    pub(crate) fn set_ty(&mut self, ty: DtcpType) {
        let byte = match ty {
            DtcpType::Transfer { drf } => {
                let ty = 0;
                let flags = if drf { 1 } else { 0 };
                ty | flags
            }
            DtcpType::Control => {
                let ty = 1 << 4;
                let flags = 0;
                ty | flags
            }
        };
        self.0.payload_mut()[0] = byte;
    }

    pub(crate) fn seq_num(&self) -> u16 {
        BigEndian::read_u16(&self.0.payload()[1..3])
    }

    pub(crate) fn set_seq_num(&mut self, seq_num: u16) {
        BigEndian::write_u16(&mut self.0.payload_mut()[1..3], seq_num)
    }
}
