use super::Connection;
use super::GetMioTcpStream;
use super::GetPeerAddr;
use super::IRC_LINE_MAX_LEN;
use super::PlaintextConnection;
use super::ReceiveMessage;
use super::SendMessage;
use irc::Message;
use irc::Result;
use mio;
use std::io::BufRead;
use std::io::BufReader;
use std::io::LineWriter;
use std::io::Write;
use std::net::SocketAddr;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

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

impl_from!(GenericConnection,
           GenericConnectionInner,
           PlaintextConnection,
           Plaintext);

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
