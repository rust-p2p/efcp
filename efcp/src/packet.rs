use crate::negotiation::Message;
use std::io::Result;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct HandshakePacket {
    negotiate: Option<Message>,
    external_addr: Option<SocketAddr>,
}

impl HandshakePacket {
    pub fn new(negotiate: Option<Message>, external_addr: Option<SocketAddr>) -> Self {
        Self {
            negotiate,
            external_addr,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        // TODO
        let negotiate = None;
        let external_addr = None;
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

    pub fn to_bytes(self) -> Vec<u8> {
        // TODO
        Vec::new()
    }
}
