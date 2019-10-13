use async_std::io::Result;
use async_std::net::UdpSocket;
use std::net::SocketAddr;

pub struct UdpEcnSocket(UdpSocket);

impl UdpEcnSocket {
    pub fn from_socket(socket: async_std::net::UdpSocket) -> Self {
        Self(socket)
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.0.local_addr()
    }

    pub fn ttl(&self) -> Result<u8> {
        let ttl = self.0.ttl()?;
        Ok(ttl as u8)
    }

    pub fn set_ttl(&self, ttl: u8) -> Result<()> {
        self.0.set_ttl(ttl as u32)
    }

    pub async fn send(&self, peer_addr: &SocketAddr, _ecn: bool, payload: &[u8]) -> Result<()> {
        self.0.send_to(payload, peer_addr).await?;
        Ok(())
    }

    pub async fn recv(&self, buffer: &mut [u8]) -> Result<(SocketAddr, usize, bool)> {
        let (len, peer_addr) = self.0.recv_from(buffer).await?;
        Ok((peer_addr, len, false))
    }
}
