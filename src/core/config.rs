use super::ErrorKind;
use super::Result;
use skimmer;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::Path;
use yamlette::book::extractor::pointer::Pointer;
use yamlette::book::extractor::traits::FromPointer;

#[derive(Debug)]
pub struct Config {
    nick: String,
    username: Option<String>,
    realname: Option<String>,
    admins: Vec<Admin>,
    server: SocketAddr,
    channels: Vec<String>,
}

#[derive(Debug)]
pub struct Admin {
    nick: Option<String>,
    user: Option<String>,
    host: Option<String>,
}

#[derive(Debug)]
pub struct ConfigBuilder(Result<Config>);

impl Config {
    pub fn try_from<T>(input: T) -> Result<Config>
        where T: IntoConfig
    {
        input.into_config()
    }

    pub fn try_from_path<P>(path: P) -> Result<Config>
        where P: AsRef<Path>
    {
        Self::try_from(File::open(path)?)
    }

    pub fn build() -> ConfigBuilder {
        Self::build_from("{nick: ''}")
    }

    pub fn build_from<T>(input: T) -> ConfigBuilder
        where T: IntoConfig
    {
        ConfigBuilder(Self::try_from(input))
    }

    pub fn build_from_path<P>(path: P) -> ConfigBuilder
        where P: AsRef<Path>
    {
        ConfigBuilder(Self::try_from_path(path))
    }
}

impl ConfigBuilder {
    pub fn nick<S>(self, nick: S) -> Self
        where S: Into<String>
    {
        let nick = nick.into();

        if nick.is_empty() {
            return ConfigBuilder(Err(ErrorKind::Config("nick".into(), "is empty".into()).into()));
        }

        ConfigBuilder(self.0.map(|cfg| Config { nick: nick, ..cfg }))
    }

    pub fn username<S>(self, username: S) -> Self
        where S: Into<String>
    {
        ConfigBuilder(self.0
                          .map(|cfg| {
                                   Config {
                                       username: Some(username.into()),
                                       ..cfg
                                   }
                               }))
    }

    pub fn realname<S>(self, realname: S) -> Self
        where S: Into<String>
    {
        ConfigBuilder(self.0
                          .map(|cfg| {
                                   Config {
                                       realname: Some(realname.into()),
                                       ..cfg
                                   }
                               }))
    }
}

// TODO: Switch to `TryFrom` once rustc 1.18 is stable.
pub trait IntoConfig {
    fn into_config(self) -> Result<Config>;
}

impl IntoConfig for Config {
    fn into_config(self) -> Result<Config> {
        Ok(self)
    }
}

impl IntoConfig for Result<Config> {
    fn into_config(self) -> Result<Config> {
        self
    }
}

impl IntoConfig for ConfigBuilder {
    fn into_config(self) -> Result<Config> {
        self.0
    }
}

impl IntoConfig for &'static str {
    fn into_config(self) -> Result<Config> {
        read_config(self)
    }
}

impl IntoConfig for String {
    fn into_config(self) -> Result<Config> {
        read_config(self)
    }
}

impl<R> IntoConfig for BufReader<R>
    where R: Read
{
    fn into_config(mut self) -> Result<Config> {
        let mut text = String::new();
        self.read_to_string(&mut text)?;
        text.into_config()
    }
}

impl IntoConfig for File {
    fn into_config(self) -> Result<Config> {
        BufReader::new(self).into_config()
    }
}

// I only want to pass `&str` and `String` to `skimmer`/`yamlette`, because I dislike how readily
// it will fail silently in other cases, and that it pre-allocates 32 KiB of buffer if given a
// reader (and fails silently if the input doesn't fit).
trait AcceptableSkimmerInput {}

impl AcceptableSkimmerInput for &'static str {}

impl AcceptableSkimmerInput for String {}

fn read_config<R>(input: R) -> Result<Config>
    where R: skimmer::reader::IntoReader + AcceptableSkimmerInput,
<<R as skimmer::reader::IntoReader>::Reader as skimmer::Read>::Datum: 'static + skimmer::Datum{
    yamlette!(read; input; [[
        {
            "nick" => (nick: String),
            "username" => (username: String),
            "realname" => (realname: String),
            "admins" => (list admins: Vec<Admin>),

            // For compatibility with the `irc` crate's configuration files....
            "owners" => (list owners: Vec<String>),
            "nickname" => (nickname: String),
            "server" => (server: &str),
            "port" => (port: u16),
            "channels" => (list channels: Vec<String>)
        }
    ]]);

    let nick = nick.or(nickname)
        .ok_or(ErrorKind::Config("nick".into(), "is not specified".into()))?;

    if nick.is_empty() {
        bail!(ErrorKind::Config("nick".into(), "is empty".into()))
    }

    Ok(Config {
           nick: nick,
           username: username,
           realname: realname,
           admins: admins
               .or(owners.map(|vec| {
        vec.into_iter()
            .map(|owner_nick| {
                     Admin {
                         nick: Some(owner_nick),
                         user: None,
                         host: None,
                     }
                 })
            .collect()
    }))
               .unwrap_or(vec![]),
           server: (server.unwrap(), port.unwrap())
               .to_socket_addrs()
               .unwrap()
               .next()
               .unwrap(),
           channels: channels.unwrap(),
       })
}

impl<'a> FromPointer<'a> for Admin {
    fn from_pointer(pointer: Pointer<'a>) -> Option<Admin> {
        yamlette_reckon!(ptr; Some(pointer); {
            "nick" => (nick: String),
            "user" => (user: String),
            "host" => (host: String)
        });

        match (nick, user, host) {
            (None, None, None) => {
                error!("Admins list entry has no keys; ignoring.");
                None
            }
            (n, u, h) => {
                Some(Admin {
                         nick: n,
                         user: u,
                         host: h,
                     })
            }
        }
    }
}
