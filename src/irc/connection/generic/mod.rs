use super::Connection;
use super::GetMioTcpStream;
use super::GetPeerAddr;
use super::IRC_LINE_MAX_LEN;
use super::PlaintextConnection;
use super::ReceiveMessage;
use super::Result;
use super::SendMessage;
use irc::Message;
use mio;
use std::io::BufRead;
use std::io::BufReader;
use std::io::LineWriter;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

// TODO: add usage example.
/// A generic IRC connection.
///
/// This type can be constructed from any type implementing [`Connection`]. It uses an internal
/// `enum` type to represent [`Connection`] types from this crate without storing them on the heap
/// to create trait objects. [`Connection`] types from other crates, if supported at all, will most
/// likely be stored on the heap.
///
/// [`Connection`]: ../trait.Connection.html
#[derive(Debug)]
pub struct GenericConnection {
    inner: GenericConnectionInner,
}

#[derive(Debug)]
enum GenericConnectionInner {
    Plaintext(PlaintextConnection),
}

macro_rules! impl_from {
    ($outer:tt, $inner:tt, $src:ty, $variant:tt) => {
        impl From<$src> for $outer {
            fn from(original: $src) -> Self {
                $outer {
                    inner: $inner::$variant(original),
                }
            }
        }
    };
}

impl_from!(
    GenericConnection,
    GenericConnectionInner,
    PlaintextConnection,
    Plaintext
);

impl Connection for GenericConnection {}

impl SendMessage for GenericConnection {
    fn try_send(&mut self, msg: Message) -> Result<()> {
        match self.inner {
            GenericConnectionInner::Plaintext(ref mut conn) => conn.try_send(msg),
        }
    }
}

impl ReceiveMessage for GenericConnection {
    fn recv(&mut self) -> Result<Option<Message>> {
        match self.inner {
            GenericConnectionInner::Plaintext(ref mut conn) => conn.recv(),
        }
    }
}

impl GetPeerAddr for GenericConnection {
    fn peer_addr(&self) -> Result<SocketAddr> {
        match self.inner {
            GenericConnectionInner::Plaintext(ref conn) => conn.peer_addr(),
        }
    }
}

impl GetMioTcpStream for GenericConnection {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        match self.inner {
            GenericConnectionInner::Plaintext(ref conn) => conn.mio_tcp_stream(),
        }
    }
}
