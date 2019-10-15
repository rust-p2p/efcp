//! Defines the `Channel` trait.
#![deny(missing_docs)]
#![deny(warnings)]
use async_trait::async_trait;
use std::io::Result;

/// Channel trait is used to decouple different parts of efcp.
#[async_trait]
pub trait Channel {
    /// Packet type sent and received through channel.
    type Packet: Clone + Send;

    /// Receive a packet from the channel.
    async fn recv(&self) -> Result<Self::Packet>;

    /// Send a packet to the channel.
    async fn send(&self, packet: Self::Packet) -> Result<()>;
}
