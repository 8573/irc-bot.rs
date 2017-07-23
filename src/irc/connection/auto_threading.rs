use super::Connection;
use core::Result;
use irc::Message;
use std::io::BufRead;
use std::io::BufReader;
use std::io::LineWriter;
use std::io::Write;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::sync::mpsc;
use std::thread;

const MSG_QUEUE_SIZE: usize = 256;

pub trait SendMessage: Clone + Send {
    fn try_send(&self, Message) -> Result<()>;
}

/// Failures to send messages via a sender are returned via the corresponding receiver.
pub trait ReceiveMessage: Send + IntoIterator<Item = Result<Message>> {}

#[derive(Clone, Debug)]
pub struct PlaintextSender {
    mpsc_sender: mpsc::SyncSender<Message>,
}

#[derive(Debug)]
pub struct PlaintextReceiver {
    mpsc_receiver: mpsc::Receiver<Result<Message>>,
}

#[derive(Debug)]
pub struct PlaintextConnection(pub PlaintextSender, pub PlaintextReceiver);

pub struct IntoIter {
    mpsc_iter: mpsc::IntoIter<Result<Message>>,
}

impl PlaintextConnection {
    pub fn from_addr<A>(server_addrs: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Self::from_tcp_stream(TcpStream::connect(server_addrs)?)
    }

    pub fn from_tcp_stream(tcp_stream: TcpStream) -> Result<Self> {
        let (input_sender, input_receiver) = mpsc::sync_channel::<Result<Message>>(MSG_QUEUE_SIZE);
        let (output_sender, output_receiver) = mpsc::sync_channel::<Message>(MSG_QUEUE_SIZE);
        let error_sender = input_sender.clone();
        let peer_addr = tcp_stream.peer_addr()?;
        let input_thread_name = format!("irc-recv::{}", peer_addr);
        let output_thread_name = format!("irc-send::{}", peer_addr);
        let mut tcp_writer = LineWriter::with_capacity(1024, tcp_stream.try_clone()?);
        let tcp_reader = BufReader::new(tcp_stream);

        thread::Builder::new().name(input_thread_name).spawn(
            move || {
                for msg in tcp_reader.lines() {
                    let msg = msg.map_err(Into::into).and_then(|s| {
                        Message::try_from(s).map_err(Into::into)
                    });
                    match input_sender.try_send(msg) {
                        Ok(()) => {}
                        Err(err) => {
                            error!(
                                "Failed to pass incoming message to message receiver: {:?}",
                                err
                            )
                        }
                    }
                }
            },
        )?;

        thread::Builder::new().name(output_thread_name).spawn(
            move || {
                for msg in output_receiver {
                    let msg = msg.raw_message();

                    match tcp_writer.write_all(msg.as_bytes()) {
                        Ok(()) => {
                    trace!("Sent message {:?}.", msg)
                }
                        Err(err) => {
                            match error_sender.try_send(Err(err.into())) {
                                Ok(()) => {
                    trace!("Failed to send message {:?}. Reported error to message receiver.", msg)
                        }
                                Err(mpsc::TrySendError::Full(Err(err))) => {
                                    error!(
                                        "Failed to send message {:?}. Failed to report error to \
                                         message receiver (MPSC queue full). Original error: {:?}",
                                        msg,
                                        err
                                    )
                                }
                                Err(mpsc::TrySendError::Disconnected(Err(err))) => {
                                    error!(
                                        "Failed to send message {:?}. Failed to report error to \
                                         message receiver (receiver disconnected). Original error: \
                                         {:?}",
                                        msg,
                                        err
                                    )
                                }
                                Err(mpsc::TrySendError::Full(Ok(_))) |
                                Err(mpsc::TrySendError::Disconnected(Ok(_))) => unreachable!(),
                            }
                        }
                    }
                }
            },
        )?;

        Ok(PlaintextConnection(
            PlaintextSender { mpsc_sender: output_sender },
            PlaintextReceiver { mpsc_receiver: input_receiver },
        ))
    }
}

impl Connection for PlaintextConnection {
    type Sender = PlaintextSender;
    type Receiver = PlaintextReceiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.0, self.1)
    }
}

impl SendMessage for PlaintextSender {
    fn try_send(&self, msg: Message) -> Result<()> {
        self.mpsc_sender.try_send(msg).map_err(Into::into)
    }
}

impl ReceiveMessage for PlaintextReceiver {}

impl IntoIterator for PlaintextReceiver {
    type Item = Result<Message>;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { mpsc_iter: self.mpsc_receiver.into_iter() }
    }
}

impl Iterator for IntoIter {
    type Item = Result<Message>;

    fn next(&mut self) -> Option<Self::Item> {
        self.mpsc_iter.next()
    }
}
