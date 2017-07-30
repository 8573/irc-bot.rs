use irc::Message;
use irc::client::Result;
use irc::connection;
use irc::connection::GenericConnection;
use irc::connection::GetMioTcpStream;
use irc::connection::prelude::*;
use mio;
use pircolate;
use std;
use std::borrow::Cow;
use std::fmt;
use std::net::SocketAddr;

lazy_static! {
    static ref DEFAULT_REALNAME: String = format!("Connected with <{url}> v{ver}",
                                                  url = env!("CARGO_PKG_HOMEPAGE"),
                                                  ver = env!("CARGO_PKG_VERSION"));
}

#[derive(Builder, Debug)]
// `#[builder(pattern = "owned")]` is necessary because `Connection`s aren't necessarily
// `Clone`able.
#[builder(pattern = "owned")]
#[builder(setter(into))]
pub struct Session<Conn>
where
    Conn: Connection,
{
    connection: Conn,

    // TODO: Try to turn these `String` fields into `Cow`s.
    nickname: String,

    #[builder(default = "self.default_username()?")]
    username: String,

    #[builder(default = "DEFAULT_REALNAME.clone()")]
    realname: String,
}

impl<Conn> SessionBuilder<Conn>
where
    Conn: Connection,
{
    pub fn new() -> Self {
        SessionBuilder {
            connection: None,
            nickname: None,
            username: None,
            realname: None,
        }
    }

    fn default_username(&self) -> std::result::Result<String, String> {
        self.nickname.clone().ok_or(
            "The `nickname` field must be set \
             regardless."
                .to_owned(),
        )
    }
}

impl<Conn> Session<Conn>
where
    Conn: Connection,
{
    pub fn start(&mut self) -> Result<()>
    where
        Conn: fmt::Debug,
    {
        let &mut Session {
            connection: _,
            ref nickname,
            ref username,
            ref realname,
        } = self;

        trace!(
            "[{}] Initiating session from {:?}",
            self.connection.peer_addr()?,
            self
        );

        self.connection.try_send(&pircolate::Message::try_from(
            format!("NICK {}", nickname),
        )?)?;
        self.connection.try_send(
            &pircolate::Message::try_from(format!(
                "USER {} 8 * :{}",
                username,
                realname
            ))?,
        )?;

        Ok(())
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
