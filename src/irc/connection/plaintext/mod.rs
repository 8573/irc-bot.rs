use super::Connection;
use super::GetMioTcpStream;
use super::GetPeerAddr;
use super::IRC_LINE_MAX_LEN;
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
pub struct PlaintextConnection {
    tcp_stream: BufReader<mio::net::TcpStream>,
}

impl PlaintextConnection {
    pub fn from_addr<A>(server_addrs: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Self::from_tcp_stream(TcpStream::connect(server_addrs)?)
    }

    pub fn from_tcp_stream(tcp_stream: TcpStream) -> Result<Self> {
        let tcp_stream = mio::net::TcpStream::from_stream(tcp_stream)?;

        trace!(
            "[{}] Established plaintext connection.",
            tcp_stream.peer_addr()?
        );

        let tcp_stream = BufReader::new(tcp_stream);

        Ok(PlaintextConnection { tcp_stream })
    }
}

impl Connection for PlaintextConnection {}

impl SendMessage for PlaintextConnection {
    fn try_send(&mut self, msg: Message) -> Result<()> {
        let msg = msg.raw_message();

        // TODO: Using `write!`/`write_fmt` here incurs at least two system calls, one to send the
        // `msg` and one to send the `"\r\n"`. `format!`-ing the `msg` and CR-LF into a `String`
        // first, incurring allocation instead, may be preferable?
        write!(self.tcp_stream.get_mut(), "{}\r\n", msg)?;

        match self.tcp_stream.get_mut().flush() {
            Ok(()) => debug!("Sent message: {:?}", msg),
            Err(err) => {
                error!(
                    "Wrote but failed to flush message: {:?} (error: {})",
                    msg,
                    err
                );
                bail!(err)
            }
        }

        Ok(())
    }
}

impl ReceiveMessage for PlaintextConnection {
    fn recv(&mut self) -> Result<Option<Message>> {
        let mut line = String::new();

        let bytes_read = self.tcp_stream.read_line(&mut line)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        // TODO: Use trim_matches once Message doesn't need an owned string.
        while line.ends_with("\n") || line.ends_with("\r") {
            let _popped_char = line.pop();
        }

        debug!("Received message: {:?}", line);

        Message::try_from(line).map(Some).map_err(Into::into)
    }
}

impl GetPeerAddr for PlaintextConnection {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.tcp_stream.get_ref().peer_addr().map_err(Into::into)
    }
}

impl GetMioTcpStream for PlaintextConnection {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        self.tcp_stream.get_ref()
    }
}
