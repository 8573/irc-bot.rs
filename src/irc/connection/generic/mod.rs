use super::Connection;
use super::GetMioTcpStream;
use super::GetPeerAddr;
use super::IRC_LINE_MAX_LEN;
use super::IntoIter;
use super::PlaintextConnection;
use super::PlaintextReceiver;
use super::PlaintextSender;
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

/// TODO: Use pub_restricted once I get 1.18.
#[derive(Debug)]
pub struct GenericConnection {
    pub sender: GenericSender,
    pub receiver: GenericReceiver,
}

#[derive(Debug)]
pub struct GenericSender {
    inner: GenericSenderInner,
}

#[derive(Debug)]
pub struct GenericReceiver {
    inner: GenericReceiverInner,
}

#[derive(Debug)]
enum GenericSenderInner {
    Plaintext(PlaintextSender),
}

#[derive(Debug)]
enum GenericReceiverInner {
    Plaintext(PlaintextReceiver),
}

macro_rules! impl_from {
    ($outer:tt, $src:ty) => {
        impl From<$src> for $outer {
            fn from(original: $src) -> Self {
                let (sender, receiver) = original.split();

                $outer {
                    sender: sender.into(),
                    receiver: receiver.into(),
                }
            }
        }
    };
    ($outer:tt, $src:ty, $inner:tt, $variant:tt) => {
        impl From<$src> for $outer {
            fn from(original: $src) -> Self {
                $outer {
                    inner: $inner::$variant(original),
                }
            }
        }
    };
}

impl_from!(GenericConnection, PlaintextConnection);
impl_from!(GenericSender,
           PlaintextSender,
           GenericSenderInner,
           Plaintext);
impl_from!(GenericReceiver,
           PlaintextReceiver,
           GenericReceiverInner,
           Plaintext);

impl Connection for GenericConnection {
    type Sender = GenericSender;
    type Receiver = GenericReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.sender, self.receiver)
    }
}

impl SendMessage for GenericSender {
    fn try_send(&mut self, msg: Message) -> Result<()> {
        match self.inner {
            GenericSenderInner::Plaintext(ref mut sender) => sender.try_send(msg),
        }
    }
}

impl ReceiveMessage for GenericReceiver {
    fn recv(&mut self) -> Result<Option<Message>> {
        match self.inner {
            GenericReceiverInner::Plaintext(ref mut receiver) => receiver.recv(),
        }
    }
}

impl IntoIterator for GenericReceiver {
    type Item = Result<Message>;
    type IntoIter = IntoIter<Self>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { receiver: self }
    }
}

impl GetPeerAddr for GenericConnection {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.sender.peer_addr()
    }
}

impl GetPeerAddr for GenericSender {
    fn peer_addr(&self) -> Result<SocketAddr> {
        match self.inner {
            GenericSenderInner::Plaintext(ref sender) => sender.peer_addr(),
        }
    }
}

impl GetPeerAddr for GenericReceiver {
    fn peer_addr(&self) -> Result<SocketAddr> {
        match self.inner {
            GenericReceiverInner::Plaintext(ref receiver) => receiver.peer_addr(),
        }
    }
}

impl GetMioTcpStream for GenericSender {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        match self.inner {
            GenericSenderInner::Plaintext(ref sender) => sender.mio_tcp_stream(),
        }
    }
}

impl GetMioTcpStream for GenericReceiver {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        match self.inner {
            GenericReceiverInner::Plaintext(ref receiver) => receiver.mio_tcp_stream(),
        }
    }
}
