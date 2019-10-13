//! Uniform interface to send/recv UDP packets with ECN information.
use std::{io, net::SocketAddr};

#[cfg(unix)]
mod cmsg;
#[cfg(unix)]
mod unix;

// No ECN support
#[cfg(not(unix))]
mod fallback;

pub trait UdpExt {
    fn init_ext(&self) -> io::Result<()>;

    fn send_ext(
        &self,
        remote: &SocketAddr,
        ecn: Option<EcnCodepoint>,
        msg: &[u8],
    ) -> io::Result<usize>;

    fn recv_ext(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr, Option<EcnCodepoint>)>;
}

/// Explicit congestion notification codepoint
#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EcnCodepoint {
    #[doc(hidden)]
    ECT0 = 0b10,
    #[doc(hidden)]
    ECT1 = 0b01,
    #[doc(hidden)]
    CE = 0b11,
}

impl EcnCodepoint {
    /// Create new object from the given bits
    pub fn from_bits(x: u8) -> Option<Self> {
        use self::EcnCodepoint::*;
        Some(match x & 0b11 {
            0b10 => ECT0,
            0b01 => ECT1,
            0b11 => CE,
            _ => {
                return None;
            }
        })
    }
}
