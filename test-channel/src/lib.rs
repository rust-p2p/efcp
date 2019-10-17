//! Unreliable channel for testing purposes.
#![deny(missing_docs)]
#![deny(warnings)]
use async_trait::async_trait;
use bytes::BytesMut;
use channel::{Channel, Loopback};
use rand::rngs::OsRng;
use rand::Rng;
use std::collections::VecDeque;
use std::io::Result;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct LossyLoopback {
    px: f64,
    pq: f64,
    delayed: Arc<Mutex<VecDeque<BytesMut>>>,
    loopback: Loopback,
}

impl LossyLoopback {
    fn new(px: f64, pq: f64) -> Self {
        Self {
            px,
            pq,
            delayed: Default::default(),
            loopback: Default::default(),
        }
    }
}

#[async_trait]
impl Channel for LossyLoopback {
    type Packet = BytesMut;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        let fate: f64 = OsRng.gen();
        if fate < self.px {
            self.loopback.send(packet.clone()).await?;
        }
        if fate < self.pq {
            self.delayed.lock().unwrap().push_back(packet);
        }
        Ok(())
    }

    async fn recv(&self) -> Result<Self::Packet> {
        let packet = self.loopback.recv().await?;
        loop {
            let packet = { self.delayed.lock().unwrap().pop_front() };
            if let Some(packet) = packet {
                self.loopback.send(packet).await?;
            } else {
                break;
            }
        }
        Ok(packet)
    }
}

/// Lossy channel.
pub struct LossyChannel {
    rx: LossyLoopback,
    tx: LossyLoopback,
}

#[async_trait]
impl Channel for LossyChannel {
    type Packet = BytesMut;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        self.tx.send(packet).await
    }

    async fn recv(&self) -> Result<Self::Packet> {
        self.rx.recv().await
    }
}

/// Lossy channel builder.
pub struct LossyChannelBuilder {
    px: f64,
    pq: f64,
    rx: LossyLoopback,
    tx: LossyLoopback,
}

impl LossyChannelBuilder {
    /// Creates a new lossy channel.
    ///
    /// The px parameter defines the probability of a packet getting
    /// transmitted.
    /// The pq parameter defines the probability of a packet getting
    /// queued.
    ///
    /// Through the combination of the px and pq parameters we can
    /// create channels that are reliable (px=1.0, pq=0.0), simulate
    /// network partitions (px=0.0, pq=0.0), send all packets twice
    /// (px=1.0, pq=1.0) or anything in between.
    pub fn new(px: f64, pq: f64) -> Self {
        assert!(0.0 <= px && px <= 1.0);
        assert!(0.0 <= pq && pq <= 1.0);
        Self {
            px,
            pq,
            rx: LossyLoopback::new(px, pq),
            tx: LossyLoopback::new(px, pq),
        }
    }

    /// Splits the channel into two duplex channels.
    pub fn split(self) -> (LossyChannel, LossyChannel) {
        let ch1 = LossyChannel {
            rx: self.rx.clone(),
            tx: self.tx.clone(),
        };
        let ch2 = LossyChannel {
            rx: self.tx,
            tx: self.rx,
        };
        (ch1, ch2)
    }
}

/// Enumerates the error conditions during transmission.
pub enum Tx {
    /// Successfull transmission
    Success,
    /// Packet is delayed and will be received out of order.
    Delay,
    /// Packet will be received twice.
    Duplicate,
    /// Packet is dropped.
    Drop,
}

impl LossyChannelBuilder {
    /// Returns the probability of an error condition occuring.
    pub fn probability(&self, cond: Tx) -> f64 {
        match cond {
            Tx::Success => self.px * (1.0 - self.pq),
            Tx::Delay => (1.0 - self.px) * self.pq,
            Tx::Duplicate => self.px * self.pq,
            Tx::Drop => (1.0 - self.px) * (1.0 - self.pq),
        }
    }
}

impl std::fmt::Display for LossyChannelBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "p(transmission) = {}", self.px)?;
        writeln!(f, "p(queue) = {}", self.pq)?;
        writeln!(f, "p(success) = {}", self.probability(Tx::Success))?;
        writeln!(f, "p(delay) = {}", self.probability(Tx::Delay))?;
        writeln!(f, "p(duplicate) = {}", self.probability(Tx::Duplicate))?;
        writeln!(f, "p(drop) = {}", self.probability(Tx::Drop))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use channel::BasePacket;

    async fn lossy_loopback() -> Result<()> {
        // Reliable channel
        let ch = LossyLoopback::new(1.0, 0.0);
        ch.send("ping".into()).await?;
        assert_eq!(ch.recv().await?.payload(), b"ping");

        // Network partition
        let ch = LossyLoopback::new(0.0, 0.0);
        ch.send("ping".into()).await?;

        // Every packet is duplicate
        let ch = LossyLoopback::new(1.0, 1.0);
        ch.send("ping".into()).await?;
        assert_eq!(ch.recv().await?.payload(), b"ping");
        assert_eq!(ch.recv().await?.payload(), b"ping");

        Ok(())
    }

    #[test]
    fn test_lossy_loopback() {
        task::block_on(lossy_loopback()).unwrap();
    }

    async fn lossy_channel() -> Result<()> {
        let (a, b) = LossyChannelBuilder::new(1.0, 0.0).split();
        a.send("ping".into()).await?;
        assert_eq!(b.recv().await?.payload(), b"ping");
        Ok(())
    }

    #[test]
    fn test_lossy_channel() {
        task::block_on(lossy_channel()).unwrap();
    }
}
