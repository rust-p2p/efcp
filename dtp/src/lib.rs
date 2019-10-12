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
mod socket;

use crate::socket::{Channel, OuterDtpSocket};
use async_std::io::Result;
use async_std::net::UdpSocket;
use async_std::stream::Stream;
use async_std::task::{Context, Poll};
use bytes::BytesMut;
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
/// let mut channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
/// channel.send(b"ping").await?;
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
///     channel.send(b"pong").await?;
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
    ///     channel.send(b"hello world").await?;
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
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
    /// channel.send(b"ping").await?;
    /// let response = channel.recv().await?;
    /// #
    /// # Ok(()) }) }
    /// ```
    pub fn outgoing(&self, peer_addr: SocketAddr) -> Result<DtpChannel> {
        let channel = self.socket.outgoing(peer_addr)?;
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
/// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
/// channel.send(b"ping").await?;
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
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
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
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
    /// channel.peer_addr();
    /// #
    /// # Ok(()) }) }
    pub fn peer_addr(&self) -> &SocketAddr {
        &self.channel.peer_addr
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
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
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
    /// let channel = socket.outgoing("127.0.0.1:8002".parse()?)?;
    /// channel.send(b"ping").await?;
    /// #
    /// # Ok(()) }) }
    pub async fn send(&self, packet: &[u8]) -> Result<()> {
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
    type Output = Result<BytesMut>;

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

        let channel_initiator = socket_initiator.outgoing(addr_responder)?;
        channel_initiator.send(b"ping").await?;

        let mut incoming = socket_responder.incoming();
        let channel_responder = incoming.next().await.unwrap()?;
        assert_eq!(channel_responder.peer_addr(), &addr_initiator);

        let request = channel_responder.recv().await?;
        assert_eq!(&request[..], b"ping");

        channel_responder.send(b"pong").await?;
        let response = channel_initiator.recv().await?;
        assert_eq!(&response[..], b"pong");

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

        let channel1 = socket1.outgoing(addr2)?;
        channel1.send(b"ping").await?;

        let channel2 = socket2.outgoing(addr1)?;
        let bytes = channel2.recv().await?;

        assert_eq!(&bytes[..], b"ping");

        Ok(())
    }

    #[test]
    fn test_outgoing_outgoing() {
        task::block_on(outgoing_outgoing()).unwrap();
    }
}