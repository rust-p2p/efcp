//! # EFCP over UDP implementation
//! The Error and Flow Control Protocol [EFCP][0] is part of the RINA effort to
//! create a more reliable, robust and extensible internet. The reference
//! implementation IRATI implements it as a kernel module. The goal is to make
//! EFCP available in user space for applications tied to the existing UDP/IP
//! infrastructure. To accomodate existing infrastructure several aspects of
//! the specificatin are adapted. These adaptions are noted together with their
//! rationale where they are made. The spec can be found at [EFCP][0].
//!
//! ## Implementation notes
//!
//! ### Explicit congestion notification
//! One feature of EFCP is explicit congestion notification which allows for
//! different congestion control policies to be implemented, such as the DECNET
//! binary feedback congestion control, TCP ECN congestion control or the Data
//! Center TCP congestion control. Because we rely on existing IP routing
//! infrastructure we do not have a mechanism for setting the ECN flag for
//! congestion notification. Because of that we can not support advanced
//! congestion policies.
//!
//! ### Configuration parameters
//! Since this is a user space implementation, where it is assumed that each
//! application contains it's own EFCP implementation, it is sufficient to make
//! configuration options decisions made at compile time.
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
//! TODO
//!
//! ### Security considerations
//! S3: Flow control cannot rely on trust between sender and receiver to achive
//!   it. A flow control algorithm must be incentive compatible.
//!
//! ## Secure communication
//! Secure communication must be resistant to eavesdropping and tampering.
//!
//! ### Conditions for secure communication
//! confidentiality: Messages cannot be read by an eavesdropper.
//!
//! integrity: Data was not changed during transit.
//!
//! authentication: Confirm the identity of the other party.
//!
//! replay protection: The same data cannot be delivered multiple times.
//!
//! ## References
//! [0]: http://nes.fit.vutbr.cz/ivesely/specs/uploads/RINA/EFCPSpec140124.pdf
//! [1]: Timer-Based Mechanisms in Reliable Transport Connection Management
#![deny(missing_docs)]
#![deny(warnings)]
pub mod constants;
pub mod dtp;
pub mod dtcp;
pub mod packet;
