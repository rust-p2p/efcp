//! Constants of the EFCP protocol.
//!
//! # Sequence number length
//! Assuming no adversary in a partially synchronous network, the minimum
//! length of a sequence number is determined by the reliability property A1.
//!
//!   2^n > (2MPL + R + A) * T
//!
//! where
//!   MPL: Maximum PDU lifetime
//!   R: Maximum time for retries
//!   A: Maximum time before an ack is sent
//!   T: Data rate at which sequence numbers are incremented.
//!
//! To provide replay protection the sequence number is never allowed to wrap.
//!
//! # Safe sizes for monotonic timers and sequence numbers
//! A u64 monotonic timer with nanosecond granularity is not going to wrap for
//! 584 years.
//!
//! 2^64 / 10^9 / 60 / 60 / 24 / 365.25 = 584.542
//!
//! If we send a packet with an empty payload each nano second we require a
//! throughput of 320 Tb/s.
//!
//! MIN_PACKET_LEN * 8 = 320 Tb/s
//!
//! where
//!     HEADER_LEN: IP_HEADER_LEN + UDP_HEADER_LEN + EFCP_HEADER_LEN = 40
//!
//! This means that we would have to send 320Tb/s continuously for 584.542
//! years to get a sequence number to wrap.
//!
//! In this implementation we assume u64 timers and sequence numbers and that
//! they will never wrap.
#![allow(missing_docs)]
pub type Instant = std::time::Instant;
pub type SequenceNumber = u64;
