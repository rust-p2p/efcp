//! Unreliable channel for testing purposes.
use rand::Rng;
use rand::rngs::ThreadRng;
use std::collections::VecDeque;

pub enum Tx {
    Success,
    Delay,
    Duplicate,
    Drop,
}

pub struct Channel<T> {
    px: f64,
    pq: f64,
    rx_queue: VecDeque<T>,
    ch_queue: VecDeque<T>,
    rng: ThreadRng,
}

impl<T> Channel<T> {
    pub fn new(px: f64, pq: f64) -> Self {
        assert!(0.0 <= px && px <= 1.0);
        assert!(0.0 <= pq && pq <= 1.0);
        Self {
            px,
            pq,
            rx_queue: VecDeque::new(),
            ch_queue: VecDeque::new(),
            rng: rand::thread_rng(),
        }
    }

    pub fn probability(&self, cond: Tx) -> f64 {
        match cond {
            Tx::Success => self.px * (1.0 - self.pq),
            Tx::Delay => (1.0 - self.px) * self.pq,
            Tx::Duplicate => self.px * self.pq,
            Tx::Drop => (1.0 - self.px) * (1.0 - self.pq),
        }
    }
}

impl<T: Clone> Channel<T> {
    pub fn send(&mut self, packet: T) {
        let fate: f64 = self.rng.gen();
        if fate < self.px {
            self.rx_queue.push_back(packet.clone());
        }
        if fate < self.pq {
            self.ch_queue.push_back(packet);
        }
    }

    pub fn recv(&mut self) -> Option<T> {
        if let Some(packet) = self.rx_queue.pop_front() {
            return Some(packet);
        }
        if let Some(packet) = self.ch_queue.pop_front() {
            return Some(packet);
        }
        None
    }
}

impl<T> std::fmt::Display for Channel<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        writeln!(f, "p(transmission) = {}", self.px)?;
        writeln!(f, "p(queue) = {}", self.pq)?;
        writeln!(f, "p(success) = {}", self.probability(Tx::Success))?;
        writeln!(f, "p(delay) = {}", self.probability(Tx::Delay))?;
        writeln!(f, "p(duplicate) = {}", self.probability(Tx::Duplicate))?;
        writeln!(f, "p(drop) = {}", self.probability(Tx::Drop))?;
        Ok(())
    }
}
