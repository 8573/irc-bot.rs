use super::Connection;
use super::GetMioTcpStream;
use super::GetPeerAddr;
use super::IRC_LINE_MAX_LEN;
use super::IntoIter;
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
    pub sender: PlaintextSender,
    pub receiver: PlaintextReceiver,
}

#[derive(Debug)]
pub struct PlaintextSender {
    tcp_writer: LineWriter<mio::net::TcpStream>,
}

#[derive(Debug)]
pub struct PlaintextReceiver {
    tcp_reader: BufReader<mio::net::TcpStream>,
}

impl PlaintextConnection {
    pub fn from_addr<A>(server_addrs: A) -> Result<Self>
        where A: ToSocketAddrs
    {
        Self::from_tcp_stream(TcpStream::connect(server_addrs)?)
    }

    pub fn from_tcp_stream(tcp_stream: TcpStream) -> Result<Self> {
        let tcp_stream = mio::net::TcpStream::from_stream(tcp_stream)?;

        trace!("[{}] Established plaintext connection.",
               tcp_stream.peer_addr()?);

        let tcp_writer = LineWriter::with_capacity(IRC_LINE_MAX_LEN, tcp_stream.try_clone()?);
        let tcp_reader = BufReader::new(tcp_stream);

        Ok(PlaintextConnection {
               sender: PlaintextSender { tcp_writer: tcp_writer },
               receiver: PlaintextReceiver { tcp_reader: tcp_reader },
           })
    }
}

impl Connection for PlaintextConnection {
    type Sender = PlaintextSender;
    type Receiver = PlaintextReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.sender, self.receiver)
    }
}

impl SendMessage for PlaintextSender {
    fn try_send(&mut self, msg: Message) -> Result<()> {
        let msg = msg.raw_message();

        write!(self.tcp_writer, "{}\r\n", msg)?;

        debug!("Sent message: {:?}", msg);

        Ok(())
    }
}

impl ReceiveMessage for PlaintextReceiver {
    fn recv(&mut self) -> Result<Option<Message>> {
        let mut line = String::new();

        let bytes_read = self.tcp_reader.read_line(&mut line)?;

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

impl IntoIterator for PlaintextReceiver {
    type Item = Result<Message>;
    type IntoIter = IntoIter<Self>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { receiver: self }
    }
}

impl GetPeerAddr for PlaintextConnection {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.sender.peer_addr()
    }
}

impl GetPeerAddr for PlaintextSender {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.tcp_writer
            .get_ref()
            .peer_addr()
            .map_err(Into::into)
    }
}

impl GetPeerAddr for PlaintextReceiver {
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.tcp_reader
            .get_ref()
            .peer_addr()
            .map_err(Into::into)
    }
}

impl GetMioTcpStream for PlaintextSender {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        self.tcp_writer.get_ref()
    }
}

impl GetMioTcpStream for PlaintextReceiver {
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        self.tcp_reader.get_ref()
    }
}
