//! # Rust EFCP over UDP implementation
//! The Error and Flow Control Protocol [EFCP][0] is part of the RINA effort to create
//! a more reliable, robust and extensible internet. The reference implementation
//! IRATI implements it as a kernel module. The goal is to make EFCP available in
//! user space for applications tied to the existing TCP/UDP/IP infrastructure. The
//! spec can be found at [EFCP][0].
//!
//! ## References
//! [0]: http://nes.fit.vutbr.cz/ivesely/specs/uploads/RINA/EFCPSpec140124.pdf
#![deny(missing_docs)]
#![deny(warnings)]
pub mod dtp;
pub mod packet;
