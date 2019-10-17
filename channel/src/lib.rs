//! Defines the `Channel` trait.
#![deny(missing_docs)]
#![deny(warnings)]
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::collections::VecDeque;
use std::io::Result;
use std::sync::{Arc, Mutex};

/// Channel trait is used to decouple different parts of efcp.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Packet type sent and received through channel.
    type Packet: BasePacket;

    /// Receive a packet from the channel.
    async fn recv(&self) -> Result<Self::Packet>;

    /// Send a packet to the channel.
    async fn send(&self, packet: Self::Packet) -> Result<()>;
}

/// Packet trait is used to encapsulate packets into a lower layer packet.
pub trait BasePacket: BufMut + Clone + Send {
    /// Creates a new packet for the given payload size.
    fn new(payload_len: usize) -> Self;

    /// Checks that the packet is valid.
    fn check(&self) -> Result<()>;

    /// Returns a byte slice of the payload.
    fn payload(&self) -> &[u8];

    /// Returns a mutable byte slice of the payload.
    fn payload_mut(&mut self) -> &mut [u8];

    /// Used for pretty printing the package.
    fn debug(&self, ds: &mut std::fmt::DebugStruct);
}

/// Packet trait is used to encapsulate packets into a lower layer packet.
pub trait Packet<P>: BasePacket {
    /// Parses a lower layer packet.
    fn parse(packet: P) -> Result<Self>;

    /// Returns the lower layer packet.
    fn into_packet(self) -> P;
}

impl BasePacket for BytesMut {
    fn new(payload_len: usize) -> Self {
        BytesMut::with_capacity(payload_len)
    }

    fn check(&self) -> Result<()> {
        Ok(())
    }

    fn payload(&self) -> &[u8] {
        &self[..]
    }

    fn payload_mut(&mut self) -> &mut [u8] {
        &mut self[..]
    }

    fn debug(&self, _: &mut std::fmt::DebugStruct) {}
}

/// A loopback channel.
#[derive(Clone, Default)]
pub struct Loopback(Arc<Mutex<VecDeque<BytesMut>>>);

struct RecvFuture<'a>(&'a Loopback);

impl<'a> Future for RecvFuture<'a> {
    type Output = BytesMut;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(packet) = (self.0).0.lock().unwrap().pop_front() {
            return Poll::Ready(packet);
        }
        cx.waker().clone().wake();
        Poll::Pending
    }
}

#[async_trait]
impl Channel for Loopback {
    type Packet = BytesMut;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        self.0.lock().unwrap().push_back(packet);
        Ok(())
    }

    async fn recv(&self) -> Result<Self::Packet> {
        Ok(RecvFuture(self).await)
    }
}

/// Derive common traits of a packet.
#[macro_export]
macro_rules! derive_packet {
    ($packet:ident) => {
        impl<P: BasePacket> channel::Packet<P> for $packet<P> {
            fn parse(packet: P) -> Result<Self> {
                let packet = Self(packet);
                packet.check()?;
                Ok(packet)
            }

            fn into_packet(self) -> P {
                self.0
            }
        }

        impl<P: BasePacket> std::fmt::Debug for $packet<P> {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                let mut ds = fmt.debug_struct("Packet");
                self.debug(&mut ds);
                ds.field("payload", &self.payload().len());
                ds.finish()
            }
        }

        impl<P: BasePacket> From<&[u8]> for $packet<P> {
            fn from(payload: &[u8]) -> Self {
                let mut packet = Self::new(payload.len());
                packet.put(payload);
                packet
            }
        }

        impl<P: BasePacket> From<&str> for $packet<P> {
            fn from(payload: &str) -> Self {
                Self::from(payload.as_bytes())
            }
        }

        impl<P: BasePacket> bytes::BufMut for $packet<P> {
            fn remaining_mut(&self) -> usize {
                self.0.remaining_mut()
            }

            unsafe fn advance_mut(&mut self, by: usize) {
                self.0.advance_mut(by);
            }

            unsafe fn bytes_mut(&mut self) -> &mut [u8] {
                self.0.bytes_mut()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use byteorder::{BigEndian, ByteOrder};
    use std::io::{Error, ErrorKind};

    #[derive(Clone)]
    struct DtpPacket<P: BasePacket>(P);

    impl<P: BasePacket> BasePacket for DtpPacket<P> {
        fn new(payload_len: usize) -> Self {
            let mut packet = P::new(payload_len + 1);
            packet.put_u8(0);
            Self(packet)
        }

        fn check(&self) -> Result<()> {
            if self.0.payload().len() < 1 {
                return Err(Error::new(ErrorKind::Other, "invalid message"));
            }
            Ok(())
        }

        fn payload(&self) -> &[u8] {
            &self.0.payload()[1..]
        }

        fn payload_mut(&mut self) -> &mut [u8] {
            &mut self.0.payload_mut()[1..]
        }

        fn debug(&self, ds: &mut std::fmt::DebugStruct) {
            self.0.debug(ds);
            ds.field("channel", &self.0.payload()[0]);
        }
    }

    derive_packet!(DtpPacket);

    struct DtpChannel<C>(C);

    #[async_trait]
    impl<C: Channel> Channel for DtpChannel<C> {
        type Packet = DtpPacket<C::Packet>;

        async fn send(&self, packet: Self::Packet) -> Result<()> {
            self.0.send(packet.into_packet()).await
        }

        async fn recv(&self) -> Result<Self::Packet> {
            DtpPacket::parse(self.0.recv().await?)
        }
    }

    #[derive(Clone)]
    struct DiscoPacket<P: BasePacket>(P);

    impl<P: BasePacket> BasePacket for DiscoPacket<P> {
        fn new(payload_len: usize) -> Self {
            let mut packet = P::new(payload_len + 8);
            packet.put_u64_be(0);
            Self(packet)
        }

        fn check(&self) -> Result<()> {
            if self.0.payload().len() < 8 {
                return Err(Error::new(ErrorKind::Other, "invalid message"));
            }
            Ok(())
        }

        fn payload(&self) -> &[u8] {
            &self.0.payload()[8..]
        }

        fn payload_mut(&mut self) -> &mut [u8] {
            &mut self.0.payload_mut()[8..]
        }

        fn debug(&self, ds: &mut std::fmt::DebugStruct) {
            self.0.debug(ds);
            ds.field("nonce", &self.nonce());
        }
    }

    derive_packet!(DiscoPacket);

    impl<P: BasePacket> DiscoPacket<P> {
        fn nonce(&self) -> u64 {
            BigEndian::read_u64(&self.0.payload()[..8])
        }
    }

    struct DiscoChannel<C>(C);

    #[async_trait]
    impl<C: Channel> Channel for DiscoChannel<C> {
        type Packet = DiscoPacket<C::Packet>;

        async fn send(&self, packet: Self::Packet) -> Result<()> {
            self.0.send(packet.into_packet()).await
        }

        async fn recv(&self) -> Result<Self::Packet> {
            DiscoPacket::parse(self.0.recv().await?)
        }
    }

    #[test]
    fn test_channels() {
        task::block_on(async {
            let ch = DiscoChannel(DtpChannel(Loopback::default()));
            ch.send("ping".into()).await.unwrap();
            let msg = ch.recv().await.unwrap();
            assert_eq!(msg.payload(), b"ping");
        });
    }
}
