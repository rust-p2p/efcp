//! # DTCP implementation
//! DTCP is a protocol for reliable data transmission over an unreliable
//! transport. Unlike TCP it uses bounded timers for opening and closing
//! connections, reducing the latency of initiating communication. It also
//! supports different flow and congestion control algorithms, which can
//! be adapted or optimized for the workload.
//!
//! ## Reliable communication
//! Packets are delivered in-order without duplicates or gaps.
//!
//! ### Conditions for reliable communication
//! O1: If no connection exists, no packets from a previous connection should
//!   cause a connection to be initialized and duplicate data accepted.
//! O2: If a connection exists, no packets from a previous connection should be
//!   accepted.
//! C1: Receiving side must not close until it has received all of the senders
//!   possible retransmissions and can unambiguously respond to them.
//! C2: Sending side must not close until it has received acks for all it's
//!   transmitted data or allowed time for an ack of it's final retransmission
//!   to return before reporting a failure.
//! A1: Sequence number of a packet used for assurance is never reused while one
//!   or more copies of that unit or it's ack exist.
//! A2: The assurance state or redundant information maintained at each end is
//!   never lost or damaged.
//! A3: Assurance control information transmitted between each end is itself
//!   perfectly error controlled.
//!
//! ### Security considerations
//! S1: Opening a connection causes state to be allocated. This can lead to DoS
//!   if an attacker opens connections in an attempt to consume enough server
//!   resources to make the system unresponsive to legitimate traffic.
//! S2: Injection of packets into a stream, can cause an illegitimate packet to
//!   be returned while ignoring the legitimate packet with the same sequence
//!   number as a duplicate.
//!
//! ## Flow control
//! Mechanism to avoid a fast sender overwhelming a slow receiver. These are
//! based on a sliding window or send rate.
//!
//! ## Congestion avoidance
//! Mechanism to avoid overwhelming the network.
//!
//! ### Detecting congestion
//! loss: Router drops packets proportional to congestion. The sender can detect
//!   congestion through the number of nacks or retransmission timeouts.
//! delay: The sender measures the round trip time. If the rtt decreases the
//!   sender assumes network congestion and throttles it's sending rate.
//! signal: Router explicitly sets a flag and forwards the packet. The receiver
//!   can detect congestion and notify the sender.
//!
//! ### Fairness
//! - Delay
//! - Proportional
//! - Max-min
//! - Minimum delay
//!
//! ### Security considerations
//! S3: Congestion control cannot rely on trust between sender and receiver to
//!   achive it. A congestion control algorithm must be incentive compatible.
//!
//! ## References
//! [0]: http://nes.fit.vutbr.cz/ivesely/specs/uploads/RINA/EFCPSpec140124.pdf
//! [1]: Timer-Based Mechanisms in Reliable Transport Connection Management
//#![deny(missing_docs)]
//#![deny(warnings)]
mod packet;
mod retransmission;

pub use crate::packet::{DtcpPacket, DtcpType};
use crate::retransmission::Retransmission;
use async_std::sync::Mutex;
use async_trait::async_trait;
use channel::{BasePacket, Channel, ChannelExt, Packet};
use crossbeam::atomic::AtomicCell;
use std::io::Result;
use std::time::Duration;

trait FetchMax<T> {
    fn fetch_max(&self, nval: T);
}

impl FetchMax<u16> for AtomicCell<u16> {
    fn fetch_max(&self, nval: u16) {
        loop {
            let oval = self.load();
            let mval = oval.max(nval);
            if self.compare_and_swap(oval, mval) == mval {
                break;
            }
        }
    }
}

/// Builder for dtcp channels.
#[derive(Clone, Debug)]
pub struct DtcpBuilder {
    /// Duration of inactivity before sending a keep alive.
    keep_alive: Duration,
    /// Maximum time to wait before sending an ack for a received packet.
    ///
    /// Not acking a packet immediately allows acking multiple packets
    /// simultanously or receiving packets out of order before reporting
    /// one missing.
    ack: Duration,
    /// Duration to wait before retransmitting.
    rtx: Duration,
    /// Number of retransmissions before reporting ETIMEDOUT.
    max_rtx: u8,
}

impl DtcpBuilder {
    /// Creates a new `DtcpBuilder`.
    pub fn new() -> Self {
        Self {
            keep_alive: Duration::from_secs(10),
            ack: Duration::from_millis(100),
            rtx: Duration::from_secs(1),
            max_rtx: 2,
        }
    }

    /// Duration of inactivity before sending a keep alive.
    pub fn set_keep_alive(mut self, keep_alive: Duration) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// Maximum time to wait before sending an ack for a received packet.
    ///
    /// Not acking a packet immediately allows acking multiple packets
    /// simultanously or receiving packets out of order before reporting
    /// one missing.
    pub fn set_ack(mut self, ack: Duration) -> Self {
        self.ack = ack;
        self
    }

    /// Duration to wait before retransmitting.
    pub fn set_rtx(mut self, rtx: Duration) -> Self {
        self.rtx = rtx;
        self
    }

    /// Number of retransmissions before reporting ETIMEDOUT.
    pub fn set_max_rtx(mut self, max_rtx: u8) -> Self {
        self.max_rtx = max_rtx;
        self
    }

    /// Wrapps a dtp channel in a dtcp channel.
    pub fn build_channel<C: ChannelExt + 'static>(&self, channel: C) -> DtcpChannel<C> {
        let tx = Retransmission::new(channel.sender(), self.rtx, self.max_rtx);
        DtcpChannel {
            channel,
            set_drf: AtomicCell::new(true),
            seq_num: AtomicCell::new(0),
            tx: Mutex::new(tx),
            last_ack: AtomicCell::new(0),
        }
    }
}

/// Dtcp channel.
pub struct DtcpChannel<C: ChannelExt> {
    channel: C,
    set_drf: AtomicCell<bool>,
    seq_num: AtomicCell<u16>,
    tx: Mutex<Retransmission<C>>,
    last_ack: AtomicCell<u16>,
}

impl<C: ChannelExt + 'static> DtcpChannel<C> {
    async fn send_transfer(&self, mut packet: DtcpPacket<C::Packet>) -> Result<()> {
        let drf = self.set_drf.swap(false);
        let seq_num = self.seq_num.fetch_add(1);
        println!("transfer {}", seq_num);
        packet.set_ty(DtcpType::Transfer { drf });
        packet.set_seq_num(seq_num);
        let packet = packet.into_packet();
        self.tx.lock().await.send(seq_num, packet)?;
        Ok(())
    }

    async fn send_ack(&self, seq_num: u16) {
        println!("ack {}", seq_num);
        let mut packet = DtcpPacket::new(2);
        packet.set_ty(DtcpType::Control);
        packet.set_seq_num(seq_num);
        self.channel.send(packet.into_packet()).await.ok();
    }
}

#[async_trait]
impl<C: ChannelExt + 'static> Channel for DtcpChannel<C> {
    type Packet = DtcpPacket<<C as Channel>::Packet>;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        self.send_transfer(packet).await
    }

    async fn recv(&self) -> Result<Self::Packet> {
        loop {
            println!("loop");
            let packet = self.channel.recv().await?;
            let packet = DtcpPacket::parse(packet)?;
            match packet.ty() {
                DtcpType::Transfer { drf } => {
                    if drf {
                        self.last_ack.store(packet.seq_num());
                        self.send_ack(packet.seq_num()).await;
                        return Ok(packet);
                    }
                    if packet.seq_num() == self.last_ack.load() + 1 {
                        self.last_ack.fetch_max(packet.seq_num());
                        self.send_ack(packet.seq_num()).await;
                        return Ok(packet);
                    }
                }
                DtcpType::Control => {
                    self.tx.lock().await.ack(packet.seq_num());
                }
            }
        }
    }
}

impl<C: ChannelExt> DtcpChannel<C> {
    /// Returns the underlying channel.
    pub fn unwrap(self) -> C {
        self.channel
    }
}

impl<C: ChannelExt> core::ops::Deref for DtcpChannel<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task;
    use channel::BasePacket;
    use dtp::{DtpChannel, DtpSocket};
    use test_channel::{LossyChannel, LossyChannelBuilder};

    fn setup_mock(
        dtcp: DtcpBuilder,
        px: f64,
        pq: f64,
    ) -> (DtcpChannel<LossyChannel>, DtcpChannel<LossyChannel>) {
        let (a, b) = LossyChannelBuilder::new(px, pq).split();
        let a = dtcp.build_channel(a);
        let b = dtcp.build_channel(b);
        (a, b)
    }

    fn setup_dtp(dtcp: DtcpBuilder) -> (DtcpChannel<DtpChannel>, DtcpChannel<DtpChannel>) {
        task::block_on(async {
            let s1 = DtpSocket::bind("127.0.0.1:0".parse().unwrap(), 1, 1)
                .await
                .unwrap();
            let s2 = DtpSocket::bind("127.0.0.1:0".parse().unwrap(), 1, 1)
                .await
                .unwrap();
            let a = dtcp.build_channel(s1.outgoing(s2.local_addr().unwrap(), 0).unwrap());
            let b = dtcp.build_channel(s2.outgoing(s1.local_addr().unwrap(), 0).unwrap());
            (a, b)
        })
    }

    async fn single_packet<C: ChannelExt + 'static>(
        a: DtcpChannel<C>,
        b: DtcpChannel<C>,
    ) -> Result<()> {
        a.send("ping".into()).await?;
        let packet = b.recv().await?;
        assert_eq!(packet.payload(), b"ping");
        Ok(())
    }

    #[test]
    fn test_mock_reliable() {
        let dtcp = DtcpBuilder::new();
        let (a, b) = setup_mock(dtcp, 1.0, 0.0);
        task::block_on(single_packet(a, b)).unwrap();
    }

    #[test]
    fn test_mock_unreliable() {
        let dtcp = DtcpBuilder::new()
            .set_rtx(Duration::from_millis(200))
            .set_max_rtx(10);
        let (a, b) = setup_mock(dtcp, 0.1, 0.1);
        task::block_on(single_packet(a, b)).unwrap();
    }

    /*#[test]
    fn test_mock_partition() {
        let dtcp = DtcpBuilder::new();
        let (a, b) = setup_mock(dtcp, 0.0, 0.0);
        task::block_on(single_packet(a, b)).unwrap();
    }*/

    #[test]
    fn test_dtp() {
        let dtcp = DtcpBuilder::new();
        let (a, b) = setup_dtp(dtcp);
        task::block_on(single_packet(a, b)).unwrap();
    }
}
