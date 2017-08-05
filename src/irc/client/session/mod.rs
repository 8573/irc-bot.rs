use irc::Message;
use irc::client::Result;
use irc::connection;
use irc::connection::GenericConnection;
use irc::connection::GetMioTcpStream;
use irc::connection::prelude::*;
use mio;
use pircolate;
use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
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
    // TODO: Use string_cache.
    nickname: String,
    username: String,
    realname: String,
}

#[derive(Copy, Clone, Debug)]
pub struct SessionBuilder<
    Conn,
    ConnField = Option<Conn>,
    NicknameField = Option<String>,
    UsernameField = Option<String>,
    RealnameField = Option<String>,
> where
    Conn: Connection,
    ConnField: Into<Option<Conn>>,
    NicknameField: Into<Option<String>>,
    UsernameField: Into<Option<String>>,
    RealnameField: Into<Option<String>>,
{
    connection: ConnField,
    nickname: NicknameField,
    username: UsernameField,
    realname: RealnameField,
    _result_phantom: PhantomData<Session<Conn>>,
}

impl<Conn, ConnField, NicknameField, UsernameField, RealnameField>
    SessionBuilder<Conn, ConnField, NicknameField, UsernameField, RealnameField>
where
    Conn: Connection,
    ConnField: Into<Option<Conn>>,
    NicknameField: Into<Option<String>>,
    UsernameField: Into<Option<String>>,
    RealnameField: Into<Option<String>>,
{
    pub fn connection(
        self,
        value: Conn,
    ) -> SessionBuilder<Conn, Conn, NicknameField, UsernameField, RealnameField> {
        let SessionBuilder {
            connection: _,
            nickname,
            username,
            realname,
            _result_phantom,
        } = self;

        SessionBuilder {
            connection: value,
            nickname,
            username,
            realname,
            _result_phantom,
        }
    }

    pub fn nickname<S>(
        self,
        value: S,
    ) -> SessionBuilder<Conn, ConnField, String, UsernameField, RealnameField>
    where
        S: Into<String>,
    {
        let SessionBuilder {
            connection,
            nickname: _,
            username,
            realname,
            _result_phantom,
        } = self;

        SessionBuilder {
            connection,
            nickname: value.into(),
            username,
            realname,
            _result_phantom,
        }
    }

    pub fn username<S>(
        self,
        value: S,
    ) -> SessionBuilder<Conn, ConnField, NicknameField, String, RealnameField>
    where
        S: Into<String>,
    {
        let SessionBuilder {
            connection,
            nickname,
            username: _,
            realname,
            _result_phantom,
        } = self;

        SessionBuilder {
            connection,
            nickname,
            username: value.into(),
            realname,
            _result_phantom,
        }
    }

    pub fn realname<S>(
        self,
        value: S,
    ) -> SessionBuilder<Conn, ConnField, NicknameField, UsernameField, String>
    where
        S: Into<String>,
    {
        let SessionBuilder {
            connection,
            nickname,
            username,
            realname: _,
            _result_phantom,
        } = self;

        SessionBuilder {
            connection,
            nickname,
            username,
            realname: value.into(),
            _result_phantom,
        }
    }
}

pub fn build<Conn>() -> SessionBuilder<Conn>
where
    Conn: Connection,
{
    SessionBuilder {
        connection: None,
        nickname: None,
        username: None,
        realname: None,
        _result_phantom: Default::default(),
    }
}

impl<Conn, UsernameField, RealnameField>
    SessionBuilder<Conn, Conn, String, UsernameField, RealnameField>
where
    Conn: Connection,
    UsernameField: Into<Option<String>>,
    RealnameField: Into<Option<String>>,
    Self: fmt::Debug,
{
    pub fn start(mut self) -> Result<Session<Conn>> {
        trace!(
            "[{}] Initiating session from {:?}",
            self.connection.peer_addr()?,
            self
        );

        let SessionBuilder {
            mut connection,
            nickname,
            username,
            realname,
            _result_phantom: _,
        } = self;

        let username = username.into().unwrap_or(nickname.clone());
        let realname = realname.into().unwrap_or(DEFAULT_REALNAME.clone());

        connection.try_send(&pircolate::Message::try_from(
            format!("NICK {}", nickname),
        )?)?;
        connection.try_send(&pircolate::Message::try_from(
            format!("USER {} 8 * :{}", username, realname),
        )?)?;

        Ok(Session {
            connection,
            nickname,
            username,
            realname,
        })
    }
}

impl<Conn> Session<Conn>
where
    Conn: Connection,
{
    pub fn into_generic(self) -> Session<GenericConnection> {
        let Session {
            connection,
            nickname,
            username,
            realname,
        } = self;

        Session {
            connection: connection.into(),
            nickname,
            username,
            realname,
        }
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
