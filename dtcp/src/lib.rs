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
#![deny(missing_docs)]
#![deny(warnings)]
mod packet;

pub use crate::packet::{DtcpPacket, DtcpType};
use async_trait::async_trait;
use channel::{Channel, Packet};
use std::io::Result;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Timer {
    enable: bool,
    start: Instant,
    interval: Duration,
}

impl Timer {
    fn new(interval: Duration) -> Self {
        Self {
            enable: false,
            start: Instant::now(),
            interval,
        }
    }

    fn start(&mut self) {
        self.start = Instant::now();
        self.enable = true;
    }

    fn stop(&mut self) -> bool {
        if self.enable {
            self.enable = false;
            Instant::now() - self.start > self.interval
        } else {
            false
        }
    }
}

/// Builder for dtcp channels.
pub struct DtcpBuilder {
    mpl: Duration,
    ack: Duration,
    max_retries: u8,
}

impl DtcpBuilder {
    /// Creates a new `DtcpBuilder`.
    pub fn new() -> Self {
        Self {
            mpl: Duration::from_millis(1000),
            ack: Duration::from_millis(100),
            max_retries: 3,
        }
    }

    /// Sets the maximum packet lifetime.
    pub fn set_mpl(mut self, mpl: Duration) -> Self {
        self.mpl = mpl;
        self
    }

    /// Sets the maximum time to ack.
    pub fn set_ack(mut self, ack: Duration) -> Self {
        self.ack = ack;
        self
    }

    /// Sets the maximum number of retries.
    pub fn set_max_retries(mut self, max_retries: u8) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Wrapps a dtp channel in a dtcp channel.
    pub fn build_channel<C: Channel>(&self, channel: C) -> DtcpChannel<C> {
        let dx = 2 * self.mpl + self.ack;
        let dt = (self.max_retries + 1) as u32 * dx;
        let sit = 2 * dt;
        let rit = 3 * dt;
        DtcpChannel {
            channel,
            set_drf: AtomicBool::new(true),
            seq_num: AtomicU16::new(0),
            sit: Mutex::new(Timer::new(sit)),
            rit: Mutex::new(Timer::new(rit)),
        }
    }
}

/// Dtcp channel.
pub struct DtcpChannel<C> {
    channel: C,
    set_drf: AtomicBool,
    seq_num: AtomicU16,
    sit: Mutex<Timer>,
    rit: Mutex<Timer>,
}

#[async_trait]
impl<C: Channel> Channel for DtcpChannel<C> {
    type Packet = DtcpPacket<C::Packet>;

    async fn send(&self, mut packet: Self::Packet) -> Result<()> {
        let expired = self.sit.lock().unwrap().stop();
        let drf = self.set_drf.swap(false, Ordering::SeqCst) || expired;
        let seq_num = self.seq_num.fetch_add(1, Ordering::SeqCst);
        packet.set_ty(DtcpType::Transfer { drf });
        packet.set_seq_num(seq_num);
        self.channel.send(packet.into_packet()).await?;
        self.sit.lock().unwrap().start();
        Ok(())
    }

    async fn recv(&self) -> Result<Self::Packet> {
        let expired = self.rit.lock().unwrap().stop();
        self.set_drf.store(expired, Ordering::SeqCst);
        let packet = self.channel.recv().await?;
        let packet = DtcpPacket::parse(packet)?;
        self.rit.lock().unwrap().start();
        Ok(packet)
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
            let s1 = DtpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let s2 = DtpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let a = dtcp.build_channel(s1.outgoing(s2.local_addr().unwrap(), 0).unwrap());
            let b = dtcp.build_channel(s2.outgoing(s1.local_addr().unwrap(), 0).unwrap());
            (a, b)
        })
    }

    async fn single_packet<C: Channel>(a: DtcpChannel<C>, b: DtcpChannel<C>) -> Result<()> {
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
