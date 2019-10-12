//! # EFCP over UDP implementation
//! The Error and Flow Control Protocol [EFCP][0] is part of the RINA effort to
//! create a more reliable, robust and extensible internet. The reference
//! implementation IRATI implements it as a kernel module. The goal is to make
//! EFCP available in user space for applications tied to the existing UDP/IP
//! infrastructure. To accomodate existing infrastructure several aspects of
//! the specificatin are adapted. These adaptions are noted together with their
//! rationale where they are made. The spec can be found at [EFCP][0].
//!
//! ## Background UDP/IP
//! ### IP
//! - Addressing
//! - Relaying
//! - TTL
//! - Explicit Congestion Notification
//!
//! ### UDP
//! - Multiplexing
//! - Error detection
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
//! ## Multiplexing
//! Mechanism to have multiple data streams over the same connection.
//!
//! ## Secure communication
//! Secure communication must be resistant to eavesdropping and tampering.
//!
//! ### Conditions for secure communication
//! confidentiality: Messages cannot be read by an eavesdropper.
//! integrity: Data was not changed during transit.
//! authentication: Confirm the identity of the other party.
//! replay protection: The same data cannot be delivered multiple times.
//!
//! ## Compression
//!
//! ## References
//! [0]: http://nes.fit.vutbr.cz/ivesely/specs/uploads/RINA/EFCPSpec140124.pdf
//! [1]: Timer-Based Mechanisms in Reliable Transport Connection Management
#![deny(missing_docs)]
#![deny(warnings)]
pub mod constants;
pub mod dtcp;
pub mod dtp;
pub mod packet;
