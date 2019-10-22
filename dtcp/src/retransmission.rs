use async_std::task;
use channel::{ChannelExt as Channel, Sender};
use crossbeam::atomic::AtomicCell;
use futures_timer::Delay;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct InnerTransmission<C: Channel> {
    sender: Sender<C>,
    packet: C::Packet,
    timer: Duration,
    tx_left: AtomicCell<i8>,
    last_tx: AtomicCell<Instant>,
    acked: AtomicCell<bool>,
    timedout: Arc<AtomicCell<bool>>,
}

struct Transmission<C: Channel>(Arc<InnerTransmission<C>>);

impl<C: Channel + 'static> Transmission<C> {
    async fn send(&self) {
        if self.0.tx_left.fetch_sub(1) > 0 {
            println!("send");
            self.0.sender.send(self.0.packet.clone()).await.ok();
            self.0.last_tx.store(Instant::now());
        } else {
            println!("timeout");
            self.0.timedout.store(true);
        }
    }

    async fn send_all(self) {
        self.send().await;
        loop {
            if self.0.acked.load() {
                break;
            }
            if self.0.timedout.load() {
                break;
            }
            let now = Instant::now();
            let last_tx = self.0.last_tx.load();
            if last_tx + self.0.timer >= now {
                Delay::new(last_tx + self.0.timer - now).await;
            } else {
                self.send().await;
            }
        }
    }

    fn ack(&self) {
        self.0.acked.store(true);
    }

    async fn nack(&self) {
        self.send().await
    }
}

impl<C: Channel> Clone for Transmission<C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct Retransmission<C: Channel> {
    sender: Sender<C>,
    rtx_time: Duration,
    max_rtx: u8,
    queue: VecDeque<(u16, Transmission<C>)>,
    timedout: Arc<AtomicCell<bool>>,
}

impl<C: Channel + 'static> Retransmission<C> {
    pub fn new(sender: Sender<C>, rtx_time: Duration, max_rtx: u8) -> Self {
        Self {
            sender,
            rtx_time,
            max_rtx,
            queue: Default::default(),
            timedout: Arc::new(AtomicCell::new(false)),
        }
    }

    pub fn send(&mut self, seq_num: u16, packet: C::Packet) -> Result<()> {
        if self.timedout.load() {
            return Err(Error::new(ErrorKind::TimedOut, "transmission failed"));
        }
        let tx = Transmission(Arc::new(InnerTransmission {
            sender: self.sender.clone(),
            packet: packet,
            timer: self.rtx_time,
            acked: AtomicCell::new(false),
            last_tx: AtomicCell::new(Instant::now()),
            tx_left: AtomicCell::new(self.max_rtx as i8 + 1),
            timedout: self.timedout.clone(),
        }));
        task::spawn(tx.clone().send_all());
        self.queue.push_back((seq_num, tx));
        Ok(())
    }

    pub fn ack(&mut self, seq_num: u16) {
        self.queue.retain(|(sn, tx)| {
            if *sn <= seq_num {
                tx.ack();
                return false;
            }
            true
        });
    }

    pub async fn nack(&self, seq_num: u16) {
        for (sn, tx) in &self.queue {
            if *sn >= seq_num {
                tx.nack().await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use channel::{BasePacket, Channel, ChannelExt, Loopback};

    async fn timeout() -> Result<()> {
        let lb = Loopback::default();
        let mut rtx = Retransmission::new(lb.sender(), Duration::from_millis(1), 0);
        rtx.send(10, [10][..].into())?;
        Delay::new(Duration::from_millis(5)).await;
        assert!(rtx.send(11, [11][..].into()).is_err());
        Ok(())
    }

    #[test]
    fn test_timeout() {
        task::block_on(timeout()).unwrap();
    }

    async fn transmission() -> Result<()> {
        let lb = Loopback::default();
        let mut rtx = Retransmission::new(lb.sender(), Duration::from_millis(1), 0);
        rtx.send(20, [20][..].into())?;
        let msg = lb.recv().await?;
        assert_eq!(msg.payload(), [20]);
        rtx.ack(20);
        Delay::new(Duration::from_millis(5)).await;
        assert!(rtx.send(21, [21][..].into()).is_ok());
        Ok(())
    }

    #[test]
    fn test_transmission() {
        task::block_on(transmission()).unwrap();
    }

    async fn retransmission() -> Result<()> {
        let lb = Loopback::default();
        let mut rtx = Retransmission::new(lb.sender(), Duration::from_millis(1), 1);
        rtx.send(30, [30][..].into())?;
        let msg = lb.recv().await?;
        assert_eq!(msg.payload(), [30]);
        let msg = lb.recv().await?;
        assert_eq!(msg.payload(), [30]);
        rtx.ack(30);
        Delay::new(Duration::from_millis(5)).await;
        assert!(rtx.send(31, [31][..].into()).is_ok());
        Ok(())
    }

    #[test]
    fn test_retransmission() {
        task::block_on(retransmission()).unwrap();
    }

    async fn nack() -> Result<()> {
        let lb = Loopback::default();
        let mut rtx = Retransmission::new(lb.sender(), Duration::from_millis(1), 1);
        rtx.send(40, [40][..].into())?;
        let msg = lb.recv().await?;
        assert_eq!(msg.payload(), [40]);
        rtx.nack(40).await;
        let msg = lb.recv().await?;
        assert_eq!(msg.payload(), [40]);
        rtx.ack(40);
        Delay::new(Duration::from_millis(5)).await;
        assert!(rtx.send(41, [41][..].into()).is_ok());
        Ok(())
    }

    #[test]
    fn test_nack() {
        task::block_on(nack()).unwrap();
    }
}
