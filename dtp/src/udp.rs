use crate::platform::{EcnCodepoint, UdpExt};
use async_std::io::{ErrorKind, Result};
use async_std::net::UdpSocket;
use core::task::{Context, Poll};
use std::net::SocketAddr;

pub struct UdpEcnSocket(UdpSocket);

impl UdpEcnSocket {
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        socket.init_ext()?;
        Ok(Self(socket))
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

    pub fn poll_send(
        &self,
        cx: &mut Context,
        peer_addr: &SocketAddr,
        ecn: bool,
        payload: &[u8],
    ) -> Poll<Result<()>> {
        let ecn = if ecn { Some(EcnCodepoint::ECT0) } else { None };
        match self.0.send_ext(peer_addr, ecn, payload) {
            Ok(_len) => Poll::Ready(Ok(())),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // TODO use mio for waking
                cx.waker().clone().wake();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    pub fn poll_recv(
        &self,
        cx: &mut Context,
        buffer: &mut [u8],
    ) -> Poll<Result<(SocketAddr, usize, bool)>> {
        match self.0.recv_ext(buffer) {
            Ok((len, peer_addr, ecn)) => {
                let ecn = if let Some(EcnCodepoint::CE) = ecn {
                    true
                } else {
                    false
                };
                Poll::Ready(Ok((peer_addr, len, ecn)))
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                // TODO use mio for waking
                cx.waker().clone().wake();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}
