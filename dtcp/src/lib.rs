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
use async_trait::async_trait;
use channel::Channel;
use dtp::Packet;
use std::io::Result;

/// Dtcp channel.
pub struct DtcpChannel {
    channel: Box<dyn Channel<Packet = Packet> + Send + Sync>,
}

#[async_trait]
impl Channel for DtcpChannel {
    type Packet = Packet;

    async fn send(&self, packet: Self::Packet) -> Result<()> {
        self.channel.send(packet).await
    }

    async fn recv(&self) -> Result<Packet> {
        self.channel.recv().await
    }
}
