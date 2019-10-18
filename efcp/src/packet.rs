use crate::negotiation::Message;
use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandshakePacket<'a> {
    negotiate: Option<Message<'a>>,
    external_addr: Option<SocketAddr>,
}

impl<'a> HandshakePacket<'a> {
    pub fn new(negotiate: Option<Message<'static>>, external_addr: Option<SocketAddr>) -> Self {
        Self {
            negotiate,
            external_addr,
        }
    }

    fn invalid() -> Error {
        Error::new(ErrorKind::Other, "invalid handshake packet")
    }

    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        let mut i = 0;
        if bytes.get(i).is_none() {
            return Err(Self::invalid());
        }
        let ty = bytes[0];
        i += 1;
        let contains_addr = ty >> 4 > 0;
        let negotiate = match ty & 0xf {
            0 => None,
            1 => {
                if bytes.get(i).is_none() {
                    return Err(Self::invalid());
                }
                let len = bytes[i] as usize;
                i += 1;
                let i2 = i + len;
                if bytes.len() < i2 {
                    return Err(Self::invalid());
                }
                let protocol = core::str::from_utf8(&bytes[i..i2]).map_err(|_| Self::invalid())?;
                i = i2;
                Some(Message::Propose(protocol))
            }
            2 => Some(Message::Accept),
            3 => Some(Message::Fail),
            _ => return Err(Self::invalid()),
        };
        let external_addr = if contains_addr {
            if bytes.get(i).is_none() {
                return Err(Self::invalid());
            }
            let len = bytes[i] as usize;
            i += 1;
            let i2 = i + len;
            if bytes.len() < i2 {
                return Err(Self::invalid());
            }
            let external_addr = core::str::from_utf8(&bytes[i..i2]).map_err(|_| Self::invalid())?;
            let external_addr = external_addr.parse().map_err(|_| Self::invalid())?;
            Some(external_addr)
        } else {
            None
        };
        Ok(Self {
            negotiate,
            external_addr,
        })
    }

    pub fn negotiate(&mut self) -> Option<Message> {
        self.negotiate.take()
    }

    pub fn external_addr(&mut self) -> Option<SocketAddr> {
        self.external_addr.take()
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        match self.negotiate {
            None => bytes.push(0),
            Some(Message::Propose(protocol)) => {
                bytes.push(1);
                let protocol = protocol.as_bytes();
                let len = protocol.len();
                if len > core::u8::MAX as usize {
                    return Err(Self::invalid());
                }
                bytes.push(len as u8);
                bytes.extend_from_slice(protocol);
            }
            Some(Message::Accept) => bytes.push(2),
            Some(Message::Fail) => bytes.push(3),
        }
        match self.external_addr {
            None => {}
            Some(addr) => {
                let addr = addr.to_string();
                let addr = addr.as_bytes();
                let len = addr.len();
                if len > core::u8::MAX as usize {
                    return Err(Self::invalid());
                }
                bytes.push(len as u8);
                bytes.extend_from_slice(addr);
                bytes[0] |= 0xf0;
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(msg: Option<Message<'static>>, addr: Option<SocketAddr>) {
        let packet = HandshakePacket::new(msg, addr);
        let bytes = packet.to_bytes().unwrap();
        let p2 = HandshakePacket::from_bytes(&bytes).unwrap();
        assert_eq!(p2, packet);
    }

    #[test]
    fn test_serde() {
        let addrv4 = "127.0.0.1:0".parse().unwrap();
        let addrv6 = "[::1]:0".parse().unwrap();
        let protocol = "/ping/1.0";
        check(None, None);
        check(Some(Message::Propose(protocol)), None);
        check(Some(Message::Accept), None);
        check(Some(Message::Fail), None);
        check(None, Some(addrv4));
        check(None, Some(addrv6));
        check(Some(Message::Propose(protocol)), Some(addrv4));
        check(Some(Message::Propose(protocol)), Some(addrv4));
        check(Some(Message::Accept), Some(addrv6));
        check(Some(Message::Fail), Some(addrv4));
    }
}
