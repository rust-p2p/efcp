//! Addr is inspired by libp2p multiaddr.
#![deny(missing_docs)]
#![deny(warnings)]
use failure::Fail;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

/// Address of a socket.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Addr {
    ip: IpAddr,
    port: u16,
}

/// Address parse error.
#[derive(Debug, Fail)]
pub enum AddrParseError {
    /// Unknown protocol.
    #[fail(display = "Unknown protocol.")]
    UnknownProtocol,
    /// Ip address parse error.
    #[fail(display = "{}", _0)]
    Ip(std::net::AddrParseError),
    /// Port parse error.
    #[fail(display = "{}", _0)]
    Port(std::num::ParseIntError),
}

impl From<std::net::AddrParseError> for AddrParseError {
    fn from(err: std::net::AddrParseError) -> Self {
        Self::Ip(err)
    }
}

impl From<std::num::ParseIntError> for AddrParseError {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::Port(err)
    }
}

impl Addr {
    /// Returns the `SocketAddr`.
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port)
    }
}

impl FromStr for Addr {
    type Err = AddrParseError;

    fn from_str(addr: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = addr.split("/").collect();
        let mut ip = None;
        let mut port = None;
        for p in parts[1..].chunks(2).enumerate() {
            match p {
                (0, [k, v]) => {
                    ip = Some(match *k {
                        "ip4" => IpAddr::V4(v.parse()?),
                        "ip6" => IpAddr::V6(v.parse()?),
                        _ => return Err(AddrParseError::UnknownProtocol),
                    })
                }
                (1, [k, v]) => {
                    port = Some(match *k {
                        "udp" => u16::from_str(v)?,
                        _ => return Err(AddrParseError::UnknownProtocol),
                    })
                }
                _ => return Err(AddrParseError::UnknownProtocol),
            }
        }
        if ip.is_none() {
            return Err(AddrParseError::UnknownProtocol);
        }
        Ok(Self {
            ip: ip.unwrap(),
            port: port.unwrap_or(0),
        })
    }
}

/// Trait to abstract over types that can be parsed to an `Addr`.
pub trait ToAddr {
    /// Returns the addr.
    fn to_addr(self) -> Result<Addr, AddrParseError>;
}

impl ToAddr for Addr {
    fn to_addr(self) -> Result<Addr, AddrParseError> {
        Ok(self)
    }
}

impl ToAddr for &str {
    fn to_addr(self) -> Result<Addr, AddrParseError> {
        Addr::from_str(self)?.to_addr()
    }
}

impl From<SocketAddr> for Addr {
    fn from(addr: SocketAddr) -> Self {
        Addr {
            ip: addr.ip(),
            port: addr.port(),
        }
    }
}

impl std::fmt::Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.ip {
            IpAddr::V4(_) => write!(f, "/ip4/")?,
            IpAddr::V6(_) => write!(f, "/ip6/")?,
        }
        self.ip.fmt(f)?;
        write!(f, "/udp/")?;
        self.port.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(saddr: &str) {
        // parse
        let addr: Addr = saddr.parse().unwrap();
        // format
        assert!(format!("{}", addr).starts_with(saddr));
        // &str -> Addr
        let addr2 = saddr.to_addr().unwrap();
        assert_eq!(addr, addr2);
        // Addr -> Addr
        let addr2 = addr.clone().to_addr().unwrap();
        assert_eq!(addr, addr2);
        // Addr -> SocketAddr -> Addr
        let addr2: Addr = addr.socket_addr().into();
        assert_eq!(addr, addr2);
    }

    #[test]
    fn test_addr() {
        rt("/ip4/127.0.0.1/udp/0");
        rt("/ip6/::1/udp/0");
        rt("/ip4/0.0.0.0");
        rt("/ip6/::1");
    }
}
