use crate::packet::DtpPacket;
use crate::udp::UdpEcnSocket;
use async_std::io::{Error, ErrorKind, Result};
use async_std::task::{Context, Poll};
use bytes::BufMut;
use channel::BasePacket;
use crossbeam::queue::ArrayQueue;
use slab::Slab;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Mutex;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Channel {
    pub(crate) peer_addr: SocketAddr,
    pub(crate) channel_id: u8,
}

pub(crate) struct InnerDtpSocket {
    socket: UdpEcnSocket,
    connections: Mutex<Slab<ArrayQueue<DtpPacket>>>,
    channel_lookup: Mutex<HashMap<Channel, usize>>,
    channels: Mutex<HashSet<Channel>>,
    incoming: ArrayQueue<Channel>,
    rx_buf_len: usize,
}

impl InnerDtpSocket {
    pub async fn bind(addr: SocketAddr, max_conns: usize, rx_buf_len: usize) -> Result<Self> {
        let socket = UdpEcnSocket::bind(addr).await?;
        Ok(Self {
            socket,
            connections: Mutex::new(Slab::with_capacity(max_conns)),
            channel_lookup: Default::default(),
            channels: Default::default(),
            incoming: ArrayQueue::new(max_conns),
            rx_buf_len,
        })
    }

    // lock order: channel_lookup, connections
    fn connection_id(&self, channel: &Channel) -> Option<usize> {
        let mut channel_lookup = self.channel_lookup.lock().unwrap();
        if let Some(conn_id) = channel_lookup.get(channel) {
            Some(*conn_id)
        } else {
            let mut conns = self.connections.lock().unwrap();
            if conns.len() < conns.capacity() {
                let conn_id = conns.insert(ArrayQueue::new(self.rx_buf_len));
                channel_lookup.insert(channel.clone(), conn_id);
                Some(conn_id)
            } else {
                None
            }
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn ttl(&self) -> Result<u8> {
        self.socket.ttl()
    }

    pub fn set_ttl(&self, ttl: u8) -> Result<()> {
        self.socket.set_ttl(ttl)
    }

    fn poll_recv(&self, cx: &mut Context) -> Poll<Result<()>> {
        let (channel, payload) = {
            let mut packet = DtpPacket::uninitialized();
            let mut buf = unsafe { packet.bytes_mut() };
            let (peer_addr, len, ecn) = {
                match self.socket.poll_recv(cx, &mut buf) {
                    Poll::Ready(Ok(res)) => res,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            };
            unsafe { packet.set_len(len) };
            if let Err(err) = packet.check() {
                return Poll::Ready(Err(err));
            }
            let channel = Channel {
                peer_addr,
                channel_id: packet.channel(),
            };
            packet.set_ecn(ecn);
            (channel, packet)
        };

        if let Some(conn_id) = self.connection_id(&channel) {
            // drop packet if rx buf is full.
            self.connections
                .lock()
                .unwrap()
                .get_mut(conn_id)
                .unwrap()
                .push(payload)
                .ok();
            if !self.channels.lock().unwrap().contains(&channel) {
                // It's safe to unwrap because creating the
                // channel worked and the incoming queue is
                // the same size as the connection slab.
                self.incoming.push(channel.clone()).unwrap()
            }
        }

        Poll::Ready(Ok(()))
    }

    pub fn poll_incoming(&self, cx: &mut Context) -> Poll<Result<Channel>> {
        if let Some(stream) = self.incoming.pop().ok() {
            return Poll::Ready(Ok(stream));
        }
        loop {
            match self.poll_recv(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
            if let Some(channel) = self.incoming.pop().ok() {
                let mut channels = self.channels.lock().unwrap();
                if !channels.contains(&channel) {
                    channels.insert(channel.clone());
                    return Poll::Ready(Ok(channel));
                }
            }
        }
    }

    pub fn poll_channel(&self, cx: &mut Context, channel: &Channel) -> Poll<Result<DtpPacket>> {
        if let Some(conn_id) = self.connection_id(channel) {
            let conns = self.connections.lock().unwrap();
            let queue = conns.get(conn_id).unwrap();
            if let Some(packet) = queue.pop().ok() {
                return Poll::Ready(Ok(packet));
            }
        }
        loop {
            match self.poll_recv(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
            if let Some(conn_id) = self.connection_id(channel) {
                let conns = self.connections.lock().unwrap();
                let queue = conns.get(conn_id).unwrap();
                if let Some(packet) = queue.pop().ok() {
                    return Poll::Ready(Ok(packet));
                }
            }
        }
    }

    pub fn outgoing(&self, peer_addr: SocketAddr, channel_id: u8) -> Result<Channel> {
        let channel = Channel {
            peer_addr,
            channel_id,
        };
        let mut channels = self.channels.lock().unwrap();
        if channels.contains(&channel) {
            return Err(Error::new(ErrorKind::Other, "channel already taken"));
        }
        channels.insert(channel.clone());
        Ok(channel)
    }

    // lock order: channels, channel_lookup, connections
    pub fn close(&self, channel: &Channel) {
        self.channels.lock().unwrap().remove(channel);
        let conn_id = self.connection_id(channel).unwrap();
        self.channel_lookup.lock().unwrap().remove(channel);
        self.connections.lock().unwrap().remove(conn_id);
    }

    pub fn poll_send(
        &self,
        cx: &mut Context,
        channel: &Channel,
        packet: &mut DtpPacket,
    ) -> Poll<Result<()>> {
        packet.set_channel(channel.channel_id);
        self.socket
            .poll_send(cx, &channel.peer_addr, packet.ecn(), packet.bytes())
    }
}
