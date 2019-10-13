use crate::packet::Packet;
use crate::udp::UdpEcnSocket;
use async_std::io::{Error, ErrorKind, Result};
use async_std::net::UdpSocket;
use async_std::task::{Context, Poll};
use bytes::BufMut;
use pin_utils::pin_mut;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Channel {
    pub(crate) peer_addr: SocketAddr,
    pub(crate) channel_id: u8,
}

struct InnerDtpSocket {
    udp: UdpEcnSocket,
    connections: HashMap<Channel, VecDeque<Packet>>,
    channels: HashSet<Channel>,
    incoming: VecDeque<Channel>,
}

impl InnerDtpSocket {
    fn from_socket(socket: UdpSocket) -> Self {
        Self {
            udp: UdpEcnSocket::from_socket(socket),
            connections: HashMap::new(),
            channels: HashSet::new(),
            incoming: VecDeque::new(),
        }
    }

    fn rx_queue(&mut self, channel: &Channel) -> &mut VecDeque<Packet> {
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
        let (channel, payload) = {
            let socket = self.inner.lock().unwrap();
            let mut packet = Packet::uninitialized();
            let mut buf = unsafe { packet.bytes_mut() };
            let (peer_addr, len, ecn) = {
                let recv_fut = socket.udp.recv(&mut buf);
                pin_mut!(recv_fut);
                match recv_fut.poll(cx) {
                    Poll::Ready(Ok(res)) => res,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            };
            if len < 1 {
                return Poll::Ready(Err(Error::new(ErrorKind::Other, "invalid channel id")));
            }
            unsafe { packet.set_len(len) };
            let channel = Channel { peer_addr, channel_id: packet.channel() };
            packet.set_ecn(ecn);
            (channel, packet)
        };
        let mut socket = self.inner.lock().unwrap();
        socket.rx_queue(&channel).push_back(payload);
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

    pub fn poll_channel(&self, cx: &mut Context, channel: &Channel) -> Poll<Result<Packet>> {
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

    pub fn outgoing(&self, peer_addr: SocketAddr, channel_id: u8) -> Result<Channel> {
        let mut socket = self.inner.lock().unwrap();
        let channel = Channel { peer_addr, channel_id };
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

    pub async fn send(&self, channel: &Channel, mut packet: Packet) -> Result<()> {
        let socket = self.inner.lock().unwrap();
        packet.set_channel(channel.channel_id);
        socket.udp.send(&channel.peer_addr, packet.ecn(), packet.bytes()).await?;
        Ok(())
    }
}
