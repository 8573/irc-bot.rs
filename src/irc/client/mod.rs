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
use std::borrow::Cow;
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
pub struct Client<Msg>
where
    Msg: Message,
{
    // TODO: use smallvec.
    sessions: Vec<SessionEntry<Msg>>,
    mpsc_receiver: mpsc::Receiver<Action<Msg>>,
    mpsc_registration: mio::Registration,
    handle_prototype: ClientHandle<Msg>,
}

#[derive(Clone, Debug)]
pub struct ClientHandle<Msg>
where
    Msg: Message,
{
    mpsc_sender: mpsc::SyncSender<Action<Msg>>,
    readiness_setter: mio::SetReadiness,
}

#[derive(Debug)]
struct SessionEntry<Msg>
where
    Msg: Message,
{
    inner: Session<GenericConnection>,
    // TODO: use smallvec.
    output_queue: Vec<Msg>,
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

impl<Msg> Client<Msg>
where
    Msg: Message,
{
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

    pub fn handle(&self) -> ClientHandle<Msg> {
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
        MsgHandler: Fn(&MessageContext, Result<Msg>) -> Reaction<Msg>,
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

fn process_session_event<Msg, MsgHandler>(
    readiness: mio::Ready,
    session: &mut SessionEntry<Msg>,
    session_id: &SessionId,
    msg_handler: &MsgHandler,
) where
    Msg: Message,
    MsgHandler: Fn(&MessageContext, Result<Msg>) -> Reaction<Msg>,
{
    if readiness.is_writable() {
        session.is_writable = true;
    }

    if session.is_writable {
        process_writable(session, session_id);
    }

    if readiness.is_readable() {
        process_readable(session, session_id, msg_handler);
    }
}

fn process_readable<Msg, MsgHandler>(
    session: &mut SessionEntry<Msg>,
    session_id: &SessionId,
    msg_handler: &MsgHandler,
) where
    Msg: Message,
    MsgHandler: Fn(&MessageContext, Result<Msg>) -> Reaction<Msg>,
{
    let msg_ctx = MessageContext { session_id: session_id.clone() };

    loop {
        let msg = match session.inner.recv::<Msg>() {
            Ok(Some(msg)) => Ok(msg),
            Ok(None) => break,
            Err(connection::Error(connection::ErrorKind::Io(ref err), _))
                if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut].contains(&err.kind()) => {
                break
            }
            Err(err) => Err(err.into()),
        };

        let reaction = handle_message(msg_handler, &msg_ctx, msg);

        process_reaction(session, session_id, reaction);
    }
}

fn process_writable<Msg>(session: &mut SessionEntry<Msg>, session_id: &SessionId)
where
    Msg: Message,
{
    let mut msgs_consumed = 0;

    for (index, msg) in session.output_queue.iter().enumerate() {
        match session.inner.try_send(msg) {
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
                    msg.to_str_lossy(),
                    err
                )
            }
        }
    }

    session.output_queue.drain(..msgs_consumed);
}

fn handle_message<Msg, MsgHandler>(
    msg_handler: &MsgHandler,
    msg_ctx: &MessageContext,
    msg: Result<Msg>,
) -> Reaction<Msg>
where
    Msg: Message,
    MsgHandler: Fn(&MessageContext, Result<Msg>) -> Reaction<Msg>,
{
    let msg = match msg {
        Ok(msg) => {
            if msg.command_bytes() == b"PING" {
                match pong_from_ping(msg) {
                    Ok(pong) => return Reaction::RawMsg(pong),
                    Err(err) => Err(err),
                }
            } else {
                Ok(msg)
            }
        }
        Err(err) => Err(err),
    };

    msg_handler(&msg_ctx, msg)
}

fn process_reaction<Msg>(
    session: &mut SessionEntry<Msg>,
    session_id: &SessionId,
    reaction: Reaction<Msg>,
) where
    Msg: Message,
{
    match reaction {
        Reaction::None => {}
        Reaction::RawMsg(ref msg) => session.send(session_id, msg),
        Reaction::Multi(reactions) => {
            for r in reactions {
                process_reaction(session, session_id, r);
            }
        }
    }
}

fn process_mpsc_queue<Msg>(client: &mut Client<Msg>)
where
    Msg: Message,
{
    while let Ok(action) = client.mpsc_receiver.try_recv() {
        process_action(client, action)
    }
}

fn process_action<Msg>(client: &mut Client<Msg>, action: Action<Msg>)
where
    Msg: Message,
{
    match action {
        Action::None => {}
        Action::RawMsg {
            ref session_id,
            ref message,
        } => {
            let ref mut session = client.sessions[session_id.index];
            session.send(session_id, message)
        }
    }
}

// TODO: Write test cases.
fn pong_from_ping<Msg>(msg: Msg) -> Result<Msg>
where
    Msg: Message,
{
    let mut pong_bytes = msg.as_bytes().to_owned();

    // TODO: Skip over prefix and IRCv3 tags, if any, rather than assuming that the message starts
    // with the command, "PING". (<http://ircv3.net/specs/core/message-tags-3.2.html>)
    pong_bytes[1] = b'O';

    Ok(Msg::try_from(Cow::Owned(pong_bytes))?)
}

impl<Msg> ClientHandle<Msg>
where
    Msg: Message,
{
    pub fn try_send(&mut self, session_id: SessionId, message: Msg) -> Result<()> {
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

impl<Msg> SessionEntry<Msg>
where
    Msg: Message,
{
    fn send(&mut self, session_id: &SessionId, msg: &Msg) {
        match self.inner.try_send(msg) {
            Ok(()) => {
                // TODO: log the `session_id`.
            }
            Err(connection::Error(connection::ErrorKind::Io(ref err), _))
                if [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut].contains(&err.kind()) => {
                trace!(
                    "[session {}] Write would block or timed out; enqueueing message for later \
                     transmission: {:?}",
                    session_id.index,
                    msg.to_str_lossy()
                );
                self.is_writable = false;
                self.output_queue.push(msg.clone());
            }
            Err(err) => {
                error!(
                    "[session {}] Failed to send message {:?} (error: {})",
                    session_id.index,
                    msg.to_str_lossy(),
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
