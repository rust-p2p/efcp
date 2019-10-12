use async_std::io::{Error, ErrorKind, Result};
use async_std::net::UdpSocket;
use async_std::task::{Context, Poll};
use bytes::{BufMut, BytesMut};
use pin_utils::pin_mut;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Channel {
    pub(crate) peer_addr: SocketAddr,
}

struct InnerDtpSocket {
    udp: UdpSocket,
    connections: HashMap<Channel, VecDeque<BytesMut>>,
    channels: HashSet<Channel>,
    incoming: VecDeque<Channel>,
}

impl InnerDtpSocket {
    fn from_socket(socket: UdpSocket) -> Self {
        Self {
            udp: socket,
            connections: HashMap::new(),
            channels: HashSet::new(),
            incoming: VecDeque::new(),
        }
    }

    fn rx_queue(&mut self, channel: &Channel) -> &mut VecDeque<BytesMut> {
        if !self.connections.contains_key(channel) {
            self.connections.insert(channel.clone(), VecDeque::new());
        }
        self.connections.get_mut(channel).unwrap()
    }
}

#[derive(Clone)]
pub(crate) struct OuterDtpSocket {
    inner: Arc<Mutex<InnerDtpSocket>>,
}

impl OuterDtpSocket {
    pub fn from_socket(socket: UdpSocket) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerDtpSocket::from_socket(socket))),
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        let socket = self.inner.lock().unwrap();
        socket.udp.local_addr()
    }

    fn poll_recv(&self, cx: &mut Context) -> Poll<Result<()>> {
        let (peer_addr, buf) = {
            let socket = self.inner.lock().unwrap();
            let mut buf = [0u8; std::u16::MAX as usize];
            let (len, peer_addr) = {
                let recv_fut = socket.udp.recv_from(&mut buf);
                pin_mut!(recv_fut);
                match recv_fut.poll(cx) {
                    Poll::Ready(Ok(res)) => res,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            };
            let mut bytes = BytesMut::with_capacity(len);
            bytes.put(&buf[..len]);
            (peer_addr, bytes)
        };
        let channel = Channel { peer_addr };
        let mut socket = self.inner.lock().unwrap();
        socket.rx_queue(&channel).push_back(buf);
        if !socket.channels.contains(&channel) {
            socket.incoming.push_back(channel);
        }
        Poll::Ready(Ok(()))
    }

    pub fn poll_incoming(&self, cx: &mut Context) -> Poll<Result<Channel>> {
        {
            let mut socket = self.inner.lock().unwrap();
            if let Some(stream) = socket.incoming.pop_front() {
                return Poll::Ready(Ok(stream));
            }
        }
        loop {
            match self.poll_recv(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
            let mut socket = self.inner.lock().unwrap();
            if let Some(channel) = socket.incoming.pop_front() {
                if !socket.channels.contains(&channel) {
                    socket.channels.insert(channel.clone());
                    return Poll::Ready(Ok(channel));
                }
            }
        }
    }

    pub fn poll_channel(&self, cx: &mut Context, channel: &Channel) -> Poll<Result<BytesMut>> {
        {
            let mut socket = self.inner.lock().unwrap();
            if let Some(packet) = socket.rx_queue(channel).pop_front() {
                return Poll::Ready(Ok(packet));
            }
        }
        loop {
            match self.poll_recv(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
            let mut socket = self.inner.lock().unwrap();
            if let Some(packet) = socket.rx_queue(channel).pop_front() {
                return Poll::Ready(Ok(packet));
            }
        }
    }

    pub fn outgoing(&self, peer_addr: SocketAddr) -> Result<Channel> {
        let mut socket = self.inner.lock().unwrap();
        let channel = Channel { peer_addr };
        if socket.channels.contains(&channel) {
            return Err(Error::new(ErrorKind::Other, "channel already taken"));
        }
        socket.channels.insert(channel.clone());
        Ok(channel)
    }

    pub fn close(&self, channel: &Channel) {
        let mut socket = self.inner.lock().unwrap();
        socket.channels.remove(channel);
        socket.connections.remove(channel);
    }

    pub async fn send(&self, channel: &Channel, packet: &[u8]) -> Result<()> {
        let socket = self.inner.lock().unwrap();
        socket.udp.send_to(packet, channel.peer_addr).await?;
        Ok(())
    }
}
