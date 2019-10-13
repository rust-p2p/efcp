//! # Data Transfer Protocol
//! Implements a simple multiplexed stateful data transfer protocol over UDP
//! and forms the basis for building more sophisticated transport protocols.
//!
//! ## Opening and closing channels
//!
//! ## TTL
//!
//! ## ECN
//!
//! ## Comparison to TCP
//!
#![deny(missing_docs)]
#![deny(warnings)]
mod dtp;
mod packet;
mod udp;

use crate::dtp::{Channel, OuterDtpSocket};
pub use crate::packet::Packet;
use async_std::io::Result;
use async_std::net::UdpSocket;
use async_std::stream::Stream;
use async_std::task::{Context, Poll};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;

/// A DTP socket.
///
/// After creating a `DtpSocket` by `bind`ing it to a socket address, it
/// listens for incoming connections. These can be accepted by awaiting
/// elements from the async stream of `incoming` connections. Connections
/// can be initiated by creating an `outgoing` `DtpChannel`.
///
/// ## Examples
///
/// ```no_run
/// # use async_std::prelude::*;
/// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
/// #
/// use dtp::DtpSocket;
///
/// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
/// let mut channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
/// channel.send("ping".into()).await?;
/// let response = channel.recv().await?;
/// #
/// # Ok(()) }) }
/// ```
/// ```no_run
/// # use async_std::prelude::*;
/// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
/// #
/// use dtp::DtpSocket;
///
/// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
/// let mut incoming = socket.incoming();
/// while let Some(channel) = incoming.next().await {
///     let channel = channel?;
///     channel.send("pong".into()).await?;
/// }
/// #
/// # Ok(()) }) }
pub struct DtpSocket {
    socket: OuterDtpSocket,
}

impl DtpSocket {
    /// Creates a new `DtpSocket` which will be bound to the specified address.
    ///
    /// Binding with a port number of 0 will request that the OS assigns a port
    /// to the socket. The port allocated can be queried via the `local_addr`
    /// method.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::DtpSocket;
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// #
    /// # Ok(()) }) }
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        let socket = OuterDtpSocket::from_socket(socket);
        Ok(Self { socket })
    }

    /// Returns a stream of incoming connections.
    ///
    /// The stream of connections is infinite, i.e awaiting the next connection
    /// will never result in `None`.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use async_std::prelude::*;
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::DtpSocket;
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let mut incoming = socket.incoming();
    /// while let Some(channel) = incoming.next().await {
    ///     let channel = channel?;
    ///     channel.send("hello world".into()).await?;
    /// }
    /// #
    /// # Ok(()) }) }
    pub fn incoming(&self) -> Incoming {
        Incoming(self)
    }

    /// Creates a channel to a peer.
    ///
    /// Will fail if the channel was already created.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use async_std::prelude::*;
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::DtpSocket;
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
    /// channel.send("ping".into()).await?;
    /// let response = channel.recv().await?;
    /// #
    /// # Ok(()) }) }
    /// ```
    pub fn outgoing(&self, peer_addr: SocketAddr, channel: u8) -> Result<DtpChannel> {
        let channel = self.socket.outgoing(peer_addr, channel)?;
        Ok(DtpChannel {
            socket: self.socket.clone(),
            channel,
        })
    }

    /// Returns the local address that this socket is bound to.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use dtp::DtpSocket;
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// socket.local_addr()?;
    /// #
    /// # Ok(()) }) }
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    pub fn ttl(&self) -> Result<u8> {
        self.socket.ttl()
    }

    /// Sets the value for the `IP_TTL` option for this socket.
    ///
    /// This value sets the time-to-live field that is used in every packet
    /// sent from this socket.
    pub fn set_ttl(&self, ttl: u8) -> Result<()> {
        self.socket.set_ttl(ttl)
    }
}

/// A stream of incoming DTP connections.
///
/// This stream is infinite, i.e awaiting the next connection will never result
/// in `None`. It is created by the `incoming` method on `DtpSocket`.
pub struct Incoming<'a>(&'a DtpSocket);

impl<'a> Stream for Incoming<'a> {
    type Item = Result<DtpChannel>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.0.socket.poll_incoming(cx) {
            Poll::Ready(Ok(channel)) => {
                let channel = DtpChannel {
                    channel,
                    socket: self.0.socket.clone(),
                };
                Poll::Ready(Some(Ok(channel)))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A DTP channel between a local and a remote socket.
///
/// A `DtpChannel` is created by calling `outgoing` on a `DtpSocket`, or
/// by polling the `Incoming` stream created by calling `incoming`.
///
/// The connection will be closed when the channel is dropped.
///
/// ## Examples
///
/// ```no_run
/// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
/// #
/// use dtp::{DtpChannel, DtpSocket};
///
/// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
/// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
/// channel.send("ping".into()).await?;
/// let response = channel.recv().await?;
/// #
/// # Ok(()) }) }
pub struct DtpChannel {
    channel: Channel,
    socket: OuterDtpSocket,
}

impl DtpChannel {
    /// Returns the local address that this channel is connected to.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use dtp::{DtpChannel, DtpSocket};
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
    /// channel.local_addr()?;
    /// #
    /// # Ok(()) }) }
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Returns the remote address that this channel is connected to.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use dtp::{DtpChannel, DtpSocket};
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
    /// channel.peer_addr();
    /// #
    /// # Ok(()) }) }
    pub fn peer_addr(&self) -> &SocketAddr {
        &self.channel.peer_addr
    }

    /// Returns the channel id.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # use async_std::prelude::*;
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::DtpSocket;
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let mut incoming = socket.incoming();
    /// while let Some(channel) = incoming.next().await {
    ///     let channel = channel?;
    ///     if channel.channel() == 0 {
    ///         channel.send("hello world".into()).await?;
    ///     }
    /// }
    /// #
    /// # Ok(()) }) }
    pub fn channel(&self) -> u8 {
        self.channel.channel_id
    }

    /// Receives data from the channel.
    /// ## Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::{DtpChannel, DtpSocket};
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
    /// let response = channel.recv().await?;
    /// #
    /// # Ok(()) }) }
    pub fn recv(&self) -> RecvFuture {
        RecvFuture(self)
    }

    /// Sends data on the channel.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), failure::Error> { async_std::task::block_on(async {
    /// #
    /// use dtp::{DtpChannel, DtpSocket};
    ///
    /// let socket = DtpSocket::bind("127.0.0.1:8001".parse()?).await?;
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?, 0)?;
    /// channel.send("ping".into()).await?;
    /// #
    /// # Ok(()) }) }
    pub async fn send(&self, packet: Packet) -> Result<()> {
        self.socket.send(&self.channel, packet).await
    }
}

impl Drop for DtpChannel {
    fn drop(&mut self) {
        self.socket.close(&self.channel)
    }
}

/// Future resolves when data is available on the channel.
pub struct RecvFuture<'a>(&'a DtpChannel);

impl<'a> Future for RecvFuture<'a> {
    type Output = Result<Packet>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.0.socket.poll_channel(cx, &self.0.channel)
    }
}

#[cfg(test)]
mod tests {
    use crate::DtpSocket;
    use async_std::prelude::*;
    use async_std::task;
    use failure::Error;

    async fn outgoing_incoming() -> Result<(), Error> {
        let socket_responder = DtpSocket::bind("127.0.0.1:0".parse()?).await?;
        let addr_responder = socket_responder.local_addr()?;

        let socket_initiator = DtpSocket::bind("127.0.0.1:0".parse()?).await?;
        let addr_initiator = socket_initiator.local_addr()?;

        let channel_initiator = socket_initiator.outgoing(addr_responder, 0)?;
        channel_initiator.send("ping".into()).await?;

        let mut incoming = socket_responder.incoming();
        let channel_responder = incoming.next().await.unwrap()?;
        assert_eq!(channel_responder.peer_addr(), &addr_initiator);

        let packet = channel_responder.recv().await?;
        assert_eq!(packet.payload(), b"ping");

        channel_responder.send("pong".into()).await?;
        let packet = channel_initiator.recv().await?;
        assert_eq!(packet.payload(), b"pong");

        Ok(())
    }

    #[test]
    fn test_outgoing_incoming() {
        task::block_on(outgoing_incoming()).unwrap();
    }

    async fn outgoing_outgoing() -> Result<(), Error> {
        let socket1 = DtpSocket::bind("127.0.0.1:0".parse()?).await?;
        let addr1 = socket1.local_addr()?;

        let socket2 = DtpSocket::bind("127.0.0.1:0".parse()?).await?;
        let addr2 = socket2.local_addr()?;

        let channel1 = socket1.outgoing(addr2, 3)?;
        channel1.send("ping".into()).await?;

        let channel2 = socket2.outgoing(addr1, 3)?;
        let packet = channel2.recv().await?;

        assert_eq!(packet.payload(), b"ping");

        Ok(())
    }

    #[test]
    fn test_outgoing_outgoing() {
        task::block_on(outgoing_outgoing()).unwrap();
    }

    async fn ttl() -> Result<(), Error> {
        let socket = DtpSocket::bind("127.0.0.1:0".parse()?).await?;
        let ttl = socket.ttl()?;
        socket.set_ttl(ttl + 10)?;
        assert_eq!(socket.ttl()?, ttl + 10);
        Ok(())
    }

    #[test]
    fn test_ttl() {
        task::block_on(ttl()).unwrap();
    }
}
