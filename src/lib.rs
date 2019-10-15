//! # EFCP over UDP instantiation
//! The Error and Flow Control Protocol [EFCP][0] is part of the RINA effort to
//! create a more reliable, robust and extensible internet. The reference
//! implementation IRATI implements it as a kernel module. The goal is to make
//! EFCP available in user space for applications tied to the existing UDP/IP
//! infrastructure. To accomodate existing infrastructure several aspects of
//! the specificatin are adapted. These adaptions are noted together with their
//! rationale where they are made. The spec can be found at [EFCP][0].
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
//! ## References
//! [0]: http://nes.fit.vutbr.cz/ivesely/specs/uploads/RINA/EFCPSpec140124.pdf
//! [1]: Timer-Based Mechanisms in Reliable Transport Connection Management
#![deny(missing_docs)]
#![deny(warnings)]
pub mod constants;
pub mod dtcp;
pub mod dtp;
pub mod packet;
