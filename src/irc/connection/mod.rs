pub use self::err::*;
pub use self::generic::GenericConnection;
pub use self::plaintext::PlaintextConnection;
use irc::Message;
use mio;
use std::net::SocketAddr;

// TODO: Delete in split-out.
pub mod prelude {
    pub use super::Connection;
    pub use super::GetPeerAddr;
    pub use super::PlaintextConnection;
    pub use super::ReceiveMessage;
    pub use super::SendMessage;
}

mod err;
mod generic;
mod plaintext;

#[cfg(auto_send_recv_threads)]
mod auto_threading;

const IRC_LINE_MAX_LEN: usize = 1024;

pub trait Connection
    : Send + ReceiveMessage + SendMessage + GetPeerAddr + Into<GenericConnection>
    {
}

pub trait SendMessage: Send + GetPeerAddr {
    fn try_send(&mut self, Message) -> Result<()>;
}

pub trait ReceiveMessage: Send + GetPeerAddr {
    /// Must perform a blocking read. Must return `Ok(None)` if there is no message to return, and
    /// not otherwise.
    fn recv(&mut self) -> Result<Option<Message>>;
}

pub trait GetPeerAddr {
    fn peer_addr(&self) -> Result<SocketAddr>;
}

/// TODO: Use pub_restricted once I get 1.18.
pub trait GetMioTcpStream {
    /// Returns a reference to `self`'s underlying `mio::net::TcpStream`, which is intended solely
    /// for registering the `TcpStream` with a `mio::Poll`.
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream;
}
