use irc::Message;
use irc::Result;
use irc::connection::GenericConnection;
use irc::connection::prelude::*;
use std::borrow::Cow;
use std::net::SocketAddr;

lazy_static! {
    static ref DEFAULT_REALNAME: String = format!("Connected with <{url}> v{ver}",
                                                  url = env!("CARGO_PKG_HOMEPAGE"),
                                                  ver = env!("CARGO_PKG_VERSION"));
}

#[derive(Debug)]
pub struct Session<Conn>
    where Conn: Connection
{
    sender: Conn::Sender,
    receiver: Conn::Receiver,
}

#[derive(Clone, Debug)]
pub struct SessionBuilder<'nickname, 'username, 'realname> {
    nickname: Cow<'nickname, str>,
    username: Cow<'username, str>,
    realname: Cow<'realname, str>,
    initial_user_mode_request: InitialUserModeRequest,
}

bitflags! {
    struct InitialUserModeRequest: u32 {
        const INIT_UMODE_REQ_WALLOPS   = 1 << 2;
        const INIT_UMODE_REQ_INVISIBLE = 1 << 3;
    }
}

pub fn build<'nickname, 'username, 'realname>
    ()
    -> SessionBuilder<'nickname, 'username, 'realname>
{
    SessionBuilder {
        nickname: Cow::Borrowed(""),
        username: Cow::Borrowed(""),
        realname: Cow::Borrowed(&DEFAULT_REALNAME),
        initial_user_mode_request: INIT_UMODE_REQ_INVISIBLE,
    }
}

impl<'nickname, 'username, 'realname> SessionBuilder<'nickname, 'username, 'realname> {
    pub fn nickname(self, nickname: &'nickname str) -> Self {
        SessionBuilder {
            nickname: Cow::Borrowed(nickname),
            ..self
        }
    }

    pub fn username(self, username: &'username str) -> Self {
        SessionBuilder {
            username: Cow::Borrowed(username),
            ..self
        }
    }

    pub fn realname(self, realname: &'realname str) -> Self {
        SessionBuilder {
            realname: Cow::Borrowed(realname),
            ..self
        }
    }

    pub fn wallops(mut self, wallops: bool) -> Self {
        self.initial_user_mode_request
            .set(INIT_UMODE_REQ_WALLOPS, wallops);
        self
    }

    pub fn invisible(mut self, invisible: bool) -> Self {
        self.initial_user_mode_request
            .set(INIT_UMODE_REQ_INVISIBLE, invisible);
        self
    }

    pub fn start<Conn>(mut self, connection: Conn) -> Result<Session<Conn>>
        where Conn: Connection
    {
        if self.nickname.is_empty() {
            // TODO: return error.
            unimplemented!()
        }

        if self.username.is_empty() {
            self.username = self.nickname.clone().into_owned().into();
        }

        trace!("[{}] Initiating session from {:?}",
               connection.peer_addr()?,
               self);

        let SessionBuilder {
            nickname,
            username,
            realname,
            initial_user_mode_request,
        } = self;

        let (mut sender, receiver) = connection.split();

        sender
            .try_send(Message::try_from(format!("NICK {}", nickname))?)?;
        sender
            .try_send(Message::try_from(format!("USER {} {} * :{}",
                                                username,
                                                initial_user_mode_request.bits(),
                                                realname))?)?;

        Ok(Session {
               sender: sender,
               receiver: receiver,
           })
    }
}

impl<Conn> Connection for Session<Conn>
    where Conn: Connection
{
    type Sender = Conn::Sender;
    type Receiver = Conn::Receiver;

    fn split(self) -> (Self::Sender, Self::Receiver) {
        (self.sender, self.receiver)
    }
}

impl<Conn> GetPeerAddr for Session<Conn>
    where Conn: Connection
{
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.sender.peer_addr()
    }
}

impl<Conn> From<Session<Conn>> for GenericConnection
    where Conn: Connection
{
    fn from(Session { sender, receiver }: Session<Conn>) -> Self {
        GenericConnection {
            sender: sender.into(),
            receiver: receiver.into(),
        }
    }
}
