use self::action::Action;
pub use self::err::*;
pub use self::msg_ctx::MessageContext;
pub use self::reaction::Reaction;
use self::session::Session;
use irc::Message;
use irc::connection;
use irc::connection::Connection;
use irc::connection::GenericConnection;
use irc::connection::GetMioTcpStream;
use irc::connection::ReceiveMessage;
use irc::connection::SendMessage;
use mio;
use pircolate;
use std;
use std::io;
use std::io::Write;
use std::sync::mpsc;

pub mod msg_ctx;
pub mod reaction;
pub mod session;

pub mod prelude {
    pub use super::session;
    pub use super::super::Message as IrcMessage;
    pub use super::super::connection::prelude::*;
}

mod action;
mod err;

#[derive(Debug)]
pub struct Client {
    // TODO: use smallvec.
    sessions: Vec<SessionEntry>,
    mpsc_receiver: mpsc::Receiver<Action>,
    mpsc_registration: mio::Registration,
    handle_prototype: ClientHandle,
}

#[derive(Clone, Debug)]
pub struct ClientHandle {
    mpsc_sender: mpsc::SyncSender<Action>,
    readiness_setter: mio::SetReadiness,
}

#[derive(Debug)]
struct SessionEntry {
    inner: Session<GenericConnection>,
    // TODO: use smallvec.
    output_queue: Vec<Message>,
    is_writable: bool,
}

#[derive(Clone, Debug)]
pub struct SessionId {
    index: usize,
}

const MPSC_QUEUE_SIZE_LIMIT: usize = 1024;

/// Identifies the context associated with a `mio` event.
///
/// The context could be an IRC session, or it could be the MPSC queue via which the library
/// consumer can asynchronously send messages and other actions to this library.
#[derive(Debug)]
enum EventContextId {
    MpscQueue,
    Session(SessionId),
}

impl Client {
    pub fn new() -> Self {
        let sessions = Vec::new();
        let (mpsc_sender, mpsc_receiver) = mpsc::sync_channel(MPSC_QUEUE_SIZE_LIMIT);
        let (mpsc_registration, readiness_setter) = mio::Registration::new2();
        let handle_prototype = ClientHandle {
            mpsc_sender,
            readiness_setter,
        };

        Client {
            sessions,
            mpsc_receiver,
            mpsc_registration,
            handle_prototype,
        }
    }

    pub fn handle(&self) -> ClientHandle {
        self.handle_prototype.clone()
    }

    pub fn add_session<Conn>(&mut self, session: Session<Conn>) -> Result<SessionId>
    where
        Conn: Connection,
    {
        let index = self.sessions.len();

        if index == std::usize::MAX {
            // `usize::MAX` would mean that the upcoming `Vec::push` call would cause an overflow,
            // assuming the system had somehow not run out of memory.

            // TODO: return an error.
            unreachable!()
        }

        self.sessions.push(SessionEntry {
            inner: session.into_generic(),
            output_queue: Vec::new(),
            is_writable: false,
        });

        Ok(SessionId { index: index })
    }

    pub fn run<MsgHandler>(mut self, msg_handler: MsgHandler) -> Result<()>
    where
        MsgHandler: Fn(&MessageContext, Result<Message>) -> Reaction,
    {
        let poll = match mio::Poll::new() {
            Ok(p) => p,
            Err(err) => {
                error!("Failed to construct `mio::Poll`: {} ({:?})", err, err);
                bail!(err)
            }
        };

        let mut events = mio::Events::with_capacity(512);

        for (index, session) in self.sessions.iter().enumerate() {
            poll.register(
                session.inner.mio_tcp_stream(),
                EventContextId::Session(SessionId { index })
                    .to_mio_token()?,
                mio::Ready::readable() | mio::Ready::writable(),
                mio::PollOpt::edge(),
            )?
        }

        poll.register(
            &self.mpsc_registration,
            EventContextId::MpscQueue.to_mio_token()?,
            mio::Ready::readable(),
            mio::PollOpt::edge(),
        )?;

        loop {
            let _event_qty = poll.poll(&mut events, None)?;

            for event in &events {
                match event.token().into() {
                    EventContextId::MpscQueue => process_mpsc_queue(&mut self),
                    EventContextId::Session(ref session_id) => {
                        let ref mut session = self.sessions[session_id.index];
                        process_session_event(event.readiness(), session, session_id, &msg_handler)
                    }
                }
            }
        }

        Ok(())
    }
}

fn process_session_event<MsgHandler>(
    readiness: mio::Ready,
    session: &mut SessionEntry,
    session_id: &SessionId,
    msg_handler: MsgHandler,
) where
    MsgHandler: Fn(&MessageContext, Result<Message>) -> Reaction,
{
    if readiness.is_writable() {
        session.is_writable = true;
    }

    if session.is_writable {
        process_writable(session, session_id);
    }

    if readiness.is_readable() {
        process_readable(session, session_id, &msg_handler);
    }
}

fn process_readable<MsgHandler>(
    session: &mut SessionEntry,
    session_id: &SessionId,
    msg_handler: MsgHandler,
) where
    MsgHandler: Fn(&MessageContext, Result<Message>) -> Reaction,
{
    let msg_ctx = MessageContext { session_id: session_id.clone() };
    let msg_handler_with_ctx = move |m| msg_handler(&msg_ctx, m);

    loop {
        let reaction = match session.inner.recv() {
            Ok(Some(ref msg)) if msg.raw_command() == "PING" => {
                match msg.raw_message().replacen("I", "O", 1).parse() {
                    Ok(pong) => Reaction::RawMsg(pong),
                    Err(err) => msg_handler_with_ctx(Err(err.into())),
                }
            }
            Ok(Some(msg)) => msg_handler_with_ctx(Ok(msg)),
            Ok(None) => break,
            Err(connection::Error(connection::ErrorKind::Io(ref err), _))
                if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut].contains(&err.kind()) => {
                break
            }
            Err(err) => msg_handler_with_ctx(Err(err.into())),
        };

        process_reaction(session, session_id, reaction);
    }
}

fn process_writable(session: &mut SessionEntry, session_id: &SessionId) {
    let mut msgs_consumed = 0;

    for (index, msg) in session.output_queue.iter().enumerate() {
        match session.inner.try_send(msg.clone()) {
            Ok(()) => msgs_consumed += 1,
            Err(connection::Error(connection::ErrorKind::Io(ref err), _))
                if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut].contains(&err.kind()) => {
                session.is_writable = false;
                break;
            }
            Err(err) => {
                msgs_consumed += 1;
                error!(
                    "[session {}] Failed to send message {:?} (error: {})",
                    session_id.index,
                    msg.raw_message(),
                    err
                )
            }
        }
    }

    session.output_queue.drain(..msgs_consumed);
}

fn process_reaction(session: &mut SessionEntry, session_id: &SessionId, reaction: Reaction) {
    match reaction {
        Reaction::None => {}
        Reaction::RawMsg(msg) => session.send(session_id, msg),
        Reaction::Multi(reactions) => {
            for r in reactions {
                process_reaction(session, session_id, r);
            }
        }
    }
}

fn process_mpsc_queue(client: &mut Client) {
    while let Ok(action) = client.mpsc_receiver.try_recv() {
        process_action(client, action)
    }
}

fn process_action(client: &mut Client, action: Action) {
    match action {
        Action::None => {}
        Action::RawMsg {
            session_id,
            message,
        } => {
            let ref mut session = client.sessions[session_id.index];
            session.send(&session_id, message)
        }
    }
}

impl ClientHandle {
    pub fn try_send(&mut self, session_id: SessionId, message: Message) -> Result<()> {
        // Add the action to the client's MPSC queue.
        self.mpsc_sender
            .try_send(Action::RawMsg {
                session_id,
                message,
            })
            .unwrap();

        self.set_ready()?;

        Ok(())
    }

    /// Notifies the associated client that there's an action to read from the MPSC queue.
    fn set_ready(&self) -> Result<()> {
        self.readiness_setter.set_readiness(mio::Ready::readable())?;

        Ok(())
    }
}

impl SessionEntry {
    fn send(&mut self, session_id: &SessionId, msg: Message) {
        match self.inner.try_send(msg.clone()) {
            Ok(()) => {
                // TODO: log the `session_id`.
            }
            Err(connection::Error(connection::ErrorKind::Io(ref err), _))
                if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut].contains(&err.kind()) => {
                trace!(
                    "[session {}] Write would block or timed out; enqueueing message for later \
                     transmission: {:?}",
                    session_id.index,
                    msg.raw_message()
                );
                self.is_writable = false;
                self.output_queue.push(msg);
            }
            Err(err) => {
                error!(
                    "[session {}] Failed to send message {:?} (error: {})",
                    session_id.index,
                    msg.raw_message(),
                    err
                )
            }
        }
    }
}

impl EventContextId {
    fn to_mio_token(&self) -> Result<mio::Token> {
        // TODO: Use QuickCheck to test that this function is bijective.
        let token_number = match self {
            &EventContextId::MpscQueue => 0,
            // TODO: Check for overflow.
            &EventContextId::Session(SessionId { index }) => 1 + index,
        };

        Ok(mio::Token(token_number))
    }
}

// TODO: Use QuickCheck to test that conversion between `EventContextId` and `mio::Token`
// round-trips properly.
impl From<mio::Token> for EventContextId {
    fn from(mio::Token(token_number): mio::Token) -> Self {
        match token_number {
            0 => EventContextId::MpscQueue,
            n => EventContextId::Session(SessionId { index: n - 1 }),
        }
    }
}
