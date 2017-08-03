use irc::Message;
use irc::client::Result;
use irc::connection;
use irc::connection::GenericConnection;
use irc::connection::GetMioTcpStream;
use irc::connection::prelude::*;
use mio;
use pircolate;
use std::borrow::Cow;
use std::net::SocketAddr;

lazy_static! {
    static ref DEFAULT_REALNAME: String = format!("Connected with <{url}> v{ver}",
                                                  url = env!("CARGO_PKG_HOMEPAGE"),
                                                  ver = env!("CARGO_PKG_VERSION"));
}

#[derive(Debug)]
pub struct Session<Conn>
where
    Conn: Connection,
{
    connection: Conn,
}

#[derive(Clone, Debug)]
pub struct SessionBuilder<'nickname, 'username, 'realname> {
    nickname: Cow<'nickname, str>,
    username: Cow<'username, str>,
    realname: Cow<'realname, str>,
}

pub fn build<'nickname, 'username, 'realname>() -> SessionBuilder<'nickname, 'username, 'realname> {
    SessionBuilder {
        nickname: Cow::Borrowed(""),
        username: Cow::Borrowed(""),
        realname: Cow::Borrowed(&DEFAULT_REALNAME),
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

    pub fn start<Conn>(mut self, mut connection: Conn) -> Result<Session<Conn>>
    where
        Conn: Connection,
    {
        if self.nickname.is_empty() {
            // TODO: return error.
            unimplemented!()
        }

        if self.username.is_empty() {
            self.username = self.nickname.clone().into_owned().into();
        }

        trace!(
            "[{}] Initiating session from {:?}",
            connection.peer_addr()?,
            self
        );

        let SessionBuilder {
            nickname,
            username,
            realname,
        } = self;

        connection.try_send(&pircolate::Message::try_from(
            format!("NICK {}", nickname),
        )?)?;
        connection.try_send(&pircolate::Message::try_from(
            format!("USER {} 8 * :{}", username, realname),
        )?)?;

        Ok(Session { connection })
    }
}

impl<Conn> Session<Conn>
where
    Conn: Connection,
{
    pub fn into_generic(self) -> Session<GenericConnection> {
        Session { connection: self.connection.into() }
    }
}

impl<Conn> ReceiveMessage for Session<Conn>
where
    Conn: Connection,
{
    fn recv<Msg>(&mut self) -> connection::Result<Option<Msg>>
    where
        Msg: Message,
    {
        self.connection.recv()
    }
}

impl<Conn> SendMessage for Session<Conn>
where
    Conn: Connection,
{
    fn try_send<Msg>(&mut self, msg: &Msg) -> connection::Result<()>
    where
        Msg: Message,
    {
        self.connection.try_send(msg)
    }
}

impl<Conn> GetPeerAddr for Session<Conn>
where
    Conn: Connection,
{
    fn peer_addr(&self) -> connection::Result<SocketAddr> {
        self.connection.peer_addr()
    }
}

impl<Conn> GetMioTcpStream for Session<Conn>
where
    Conn: Connection + GetMioTcpStream,
{
    fn mio_tcp_stream(&self) -> &mio::net::TcpStream {
        self.connection.mio_tcp_stream()
    }
}
