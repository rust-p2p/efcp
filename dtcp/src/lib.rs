//! Implements a reliable transport.
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
