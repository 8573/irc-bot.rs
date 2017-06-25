pub use self::reaction::Reaction;
use self::session::Session;
use irc::Error;
use irc::ErrorKind;
use irc::Message;
use irc::Result;
use irc::connection::Connection;
use irc::connection::GenericReceiver;
use irc::connection::GenericSender;
use irc::connection::GetMioTcpStream;
use irc::connection::ReceiveMessage;
use mio;
use pircolate;
use std::io;
use std::io::Write;

pub mod reaction;
pub mod session;

pub mod prelude {
    pub use super::session;
    pub use super::super::Message as IrcMessage;
    pub use super::super::connection::prelude::*;
}

#[derive(Debug)]
pub struct Client {
    // TODO: use smallvec.
    sessions: Vec<SessionEntry>,
}

#[derive(Debug)]
struct SessionEntry {
    sender: GenericSender,
    receiver: GenericReceiver,
    // TODO: use smallvec.
    output_queue: Vec<Message>,
    is_writable: bool,
}

#[derive(Clone, Debug)]
pub struct SessionId {
    index: usize,
}

impl Client {
    pub fn new() -> Self {
        Client { sessions: Vec::new() }
    }

    pub fn add_session<Conn>(&mut self, session: Session<Conn>) -> Result<SessionId>
        where Conn: Connection
    {
        let (sender, receiver) = session.split();

        let index = self.sessions.len();

        self.sessions
            .push(SessionEntry {
                      sender: sender.into(),
                      receiver: receiver.into(),
                      output_queue: Vec::new(),
                      is_writable: false,
                  });

        Ok(SessionId { index: index })
    }

    pub fn run<MsgHandler>(mut self, msg_handler: MsgHandler) -> Result<()>
        where MsgHandler: Fn(Result<Message>) -> Reaction
    {
        let poll = match mio::Poll::new() {
            Ok(p) => p,
            Err(err) => {
                error!("Failed to construct `mio::Poll`: {} ({:?})", err, err);
                bail!(err)
            }
        };

        let mut events = mio::Events::with_capacity(512);

        // XXX: The sender `TcpStream`s are `try_clone`'d from the receiver `TcpStream`s. The
        // following code registers only the receiver `TcpStream`s with the mio `Poll`, and assumes
        // that if a receiver `TcpStream` becomes writable, the corresponding sender `TcpStream` is
        // also writable. This may or may not be okay to assume.

        for (index, session) in self.sessions.iter().enumerate() {
            poll.register(session.receiver.mio_tcp_stream(),
                          mio::Token(index),
                          mio::Ready::readable() | mio::Ready::writable(),
                          mio::PollOpt::edge())?
        }

        loop {
            let _event_qty = poll.poll(&mut events, None)?;

            for event in &events {
                let mio::Token(session_index) = event.token();
                let ref mut session = self.sessions[session_index];

                if event.readiness().is_writable() {
                    session.is_writable = true;
                }

                if session.is_writable {
                    let mut msgs_consumed = 0;
                    for (index, msg) in session.output_queue.iter().enumerate() {
                        match write!(session.receiver.mio_tcp_stream(),
                                     "{}\r\n",
                                     msg.raw_message()) {
                            Ok(()) => msgs_consumed += 1,
                            Err(ref err) if [io::ErrorKind::WouldBlock,
                                             io::ErrorKind::TimedOut]
                                                    .contains(&err.kind()) => {
                                session.is_writable = false;
                                break;
                            }
                            Err(err) => {
                                msgs_consumed += 1;
                                error!("[session {}] Failed to send message {:?} (error: {})",
                                       session_index,
                                       msg.raw_message(),
                                       err)
                            }
                        }
                    }
                    session.output_queue.drain(..msgs_consumed);
                }

                if event.readiness().is_readable() {
                    process_readable(session, session_index, &msg_handler);
                }
            }
        }

        Ok(())
    }
}

fn process_readable<MsgHandler>(session: &mut SessionEntry,
                                session_index: usize,
                                msg_handler: MsgHandler)
    where MsgHandler: Fn(Result<Message>) -> Reaction
{
    loop {
        let reaction = match session.receiver.recv() {
            Ok(Some(ref msg)) if msg.raw_command() == "PING" => {
                match msg.raw_message().replacen("I", "O", 1).parse() {
                    Ok(pong) => Reaction::RawMsg(pong),
                    Err(err) => msg_handler(Err(err.into())),
                }
            }
            Ok(Some(msg)) => msg_handler(Ok(msg)),
            Ok(None) => break,
            Err(Error(ErrorKind::Io(ref err), _)) if [io::ErrorKind::WouldBlock,
                                                      io::ErrorKind::TimedOut]
                                                             .contains(&err.kind()) => break,
            Err(err) => msg_handler(Err(err)),
        };

        process_reaction(session, session_index, reaction);
    }
}

fn process_reaction(session: &mut SessionEntry, session_index: usize, reaction: Reaction) {
    match reaction {
        Reaction::None => {}
        Reaction::RawMsg(msg) => session.send(session_index, msg),
        Reaction::Multi(reactions) => {
            for r in reactions {
                process_reaction(session, session_index, r);
            }
        }
    }
}

impl SessionEntry {
    fn send(&mut self, session_index: usize, msg: Message) {
        match write!(self.receiver.mio_tcp_stream(), "{}\r\n", msg.raw_message()) {
            Ok(()) => {
                match self.receiver.mio_tcp_stream().flush() {
                    Ok(()) => trace!("[session {}] Sent message: {:?}",
                                     session_index,
                                     msg.raw_message()),
                    Err(err) => error!("[session {}] Wrote but failed to flush message: {:?} \
                                       (error: {})",
                                       session_index,
                                       msg.raw_message(),
                                       err),
                }
            }
            Err(ref err) if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut]
                                .contains(&err.kind()) => {
                trace!("[session {}] Write would block or timed out; enqueueing message for \
                        later transmission: {:?}",
                       session_index,
                       msg.raw_message());
                self.is_writable = false;
                self.output_queue.push(msg);
            }
            Err(err) => {
                error!("[session {}] Failed to send message {:?} (error: {})",
                       session_index,
                       msg.raw_message(),
                       err)
            }
        }
    }
}
