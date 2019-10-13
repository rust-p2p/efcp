use crate::platform::{EcnCodepoint, UdpExt};
use async_std::io::Result;
use async_std::net::UdpSocket;
use std::net::SocketAddr;

impl UdpExt for UdpSocket {
    fn init_ext(&self) -> Result<()> {
        Ok(())
    }

    fn send_ext(&self, remote: &SocketAddr, _: bool, msg: &[u8]) -> io::Result<usize> {
        self.send_to(msg, remote)
    }

    fn recv_ext(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr, Option<EcnCodepoint>)> {
        self.recv_from(buf).map(|(x, y)| (x, y, false))
    }
}
