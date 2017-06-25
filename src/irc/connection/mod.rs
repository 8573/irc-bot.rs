pub use self::generic::GenericConnection;
pub use self::generic::GenericReceiver;
pub use self::generic::GenericSender;
pub use self::plaintext::PlaintextConnection;
pub use self::plaintext::PlaintextReceiver;
pub use self::plaintext::PlaintextSender;
use irc::Message;
use irc::Result;
use mio;
use std::net::SocketAddr;

// TODO: Delete in split-out.
pub mod prelude {
    pub use super::Connection;
    pub use super::GetPeerAddr;
    pub use super::PlaintextConnection;
    pub use super::PlaintextReceiver;
    pub use super::PlaintextSender;
    pub use super::ReceiveMessage;
    pub use super::SendMessage;
}

mod generic;
mod plaintext;

#[cfg(auto_send_recv_threads)]
mod auto_threading;

const IRC_LINE_MAX_LEN: usize = 1024;

pub trait Connection: Send + GetPeerAddr + Into<GenericConnection> {
    type Sender: SendMessage;
    type Receiver: ReceiveMessage;

    fn split(self) -> (Self::Sender, Self::Receiver);
}

pub trait SendMessage: Send + GetPeerAddr + Into<GenericSender> {
    fn try_send(&mut self, Message) -> Result<()>;
}

pub trait ReceiveMessage
    : Send + GetPeerAddr + Into<GenericReceiver> + IntoIterator<Item = Result<Message>>
    {
    /// Must perform a blocking read. Must return `Ok(None)` if there is no message to return, and
    /// not otherwise.
    fn recv(&mut self) -> Result<Option<Message>>;
}

pub trait GetPeerAddr {
    fn peer_addr(&self) -> Result<SocketAddr>;
}

/// TODO: Use pub_restricted once I get 1.18.
pub trait GetMioTcpStream {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream;
}

pub struct IntoIter<R>
    where R: ReceiveMessage
{
    receiver: R,
}

impl<R> Iterator for IntoIter<R>
    where R: ReceiveMessage
{
    type Item = Result<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.recv() {
            Ok(Some(msg)) => Some(Ok(msg)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}
