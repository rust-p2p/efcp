//! Unreliable channel for testing purposes.
#![deny(missing_docs)]
#![deny(warnings)]
use async_trait::async_trait;
use channel::Channel;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use rand::rngs::OsRng;
use rand::Rng;
use std::collections::VecDeque;
use std::io::Result;
use std::sync::{Arc, Mutex};

struct SimplexChannel<T> {
    px: f64,
    pq: f64,
    rx_queue: VecDeque<T>,
    ch_queue: VecDeque<T>,
}

impl<T: Clone> SimplexChannel<T> {
    fn new(px: f64, pq: f64) -> Self {
        Self {
            px,
            pq,
            rx_queue: VecDeque::new(),
            ch_queue: VecDeque::new(),
        }
    }

    fn send(&mut self, packet: T) {
        let fate: f64 = OsRng.gen();
        if fate < self.px {
            self.rx_queue.push_back(packet.clone());
        }
        if fate < self.pq {
            self.ch_queue.push_back(packet);
        }
    }

    fn recv(&mut self) -> Option<T> {
        if let Some(packet) = self.rx_queue.pop_front() {
            return Some(packet);
        }
        if let Some(packet) = self.ch_queue.pop_front() {
            return Some(packet);
        }
        None
    }
}

/// Duplex communication channel
pub struct DuplexChannel<T> {
    rx: Arc<Mutex<SimplexChannel<T>>>,
    tx: Arc<Mutex<SimplexChannel<T>>>,
}

struct RecvFuture<'a, T: Clone + Send>(&'a DuplexChannel<T>);

impl<'a, T: Clone + Send> Future for RecvFuture<'a, T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if let Some(packet) = self.0.rx.lock().unwrap().recv() {
            return Poll::Ready(Ok(packet));
        }
        cx.waker().clone().wake();
        Poll::Pending
    }
}

#[async_trait]
impl<T: Clone + Send> Channel for DuplexChannel<T> {
    type Packet = T;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        Ok(self.tx.lock().unwrap().send(packet))
    }

    async fn recv(&self) -> Result<Self::Packet> {
        RecvFuture(self).await
    }
}

/// Lossy channel.
pub struct LossyChannel<T> {
    px: f64,
    pq: f64,
    rx: SimplexChannel<T>,
    tx: SimplexChannel<T>,
}

impl<T: Clone> LossyChannel<T> {
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
            rx: SimplexChannel::new(px, pq),
            tx: SimplexChannel::new(px, pq),
        }
    }

    /// Splits the channel into two duplex channels.
    pub fn split(self) -> (DuplexChannel<T>, DuplexChannel<T>) {
        let rx = Arc::new(Mutex::new(self.rx));
        let tx = Arc::new(Mutex::new(self.tx));
        let ch1 = DuplexChannel {
            rx: rx.clone(),
            tx: tx.clone(),
        };
        let ch2 = DuplexChannel { rx: tx, tx: rx };
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

impl<T> LossyChannel<T> {
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

impl<T> std::fmt::Display for LossyChannel<T> {
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

    #[test]
    fn simplex() {
        // Reliable channel
        let mut ch = SimplexChannel::<u32>::new(1.0, 0.0);
        ch.send(42);
        assert_eq!(ch.recv(), Some(42));
        assert_eq!(ch.recv(), None);

        // Network partition
        let mut ch = SimplexChannel::<u32>::new(0.0, 0.0);
        ch.send(42);
        assert_eq!(ch.recv(), None);
        assert_eq!(ch.recv(), None);

        // Every packet is duplicate
        let mut ch = SimplexChannel::<u32>::new(1.0, 1.0);
        ch.send(42);
        assert_eq!(ch.recv(), Some(42));
        assert_eq!(ch.recv(), Some(42));
    }

    async fn lossy() -> Result<()> {
        let (a, b) = LossyChannel::<u32>::new(1.0, 0.0).split();
        a.send(42).await?;
        assert_eq!(b.recv().await?, 42);
        Ok(())
    }

    #[test]
    fn test_lossy() {
        task::block_on(lossy()).unwrap();
    }
}
