use super::ErrorKind;
use super::Result;
use serde_yaml;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::path::Path;

// TODO: Hide the Deserialize implementation, which doesn't validate.
#[derive(Debug, Deserialize)]
pub struct Config {
    pub(crate) nickname: String,

    #[serde(default)]
    pub(crate) username: String,

    #[serde(default)]
    pub(crate) realname: String,

    #[serde(default)]
    pub(crate) admins: Vec<Admin>,

    pub(crate) servers: Vec<Server>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Admin {
    #[serde(default)]
    pub nick: Option<String>,

    #[serde(default)]
    pub user: Option<String>,

    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Server {
    pub host: String,

    pub port: u16,

    #[serde(default = "mk_true")]
    pub tls: bool,
}

#[derive(Debug)]
pub struct ConfigBuilder(Result<Config>);

impl Config {
    pub fn try_from<T>(input: T) -> Result<Config>
    where
        T: IntoConfig,
    {
        input.into_config()
    }

    pub fn try_from_path<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        Self::try_from(File::open(path)?)
    }

    pub fn build() -> ConfigBuilder {
        Self::build_from("{nickname: ''}")
    }

    pub fn build_from<T>(input: T) -> ConfigBuilder
    where
        T: IntoConfig,
    {
        ConfigBuilder(Self::try_from(input))
    }

    pub fn build_from_path<P>(path: P) -> ConfigBuilder
    where
        P: AsRef<Path>,
    {
        ConfigBuilder(Self::try_from_path(path))
    }
}

impl Server {
    pub fn resolve(&self) -> SocketAddr {
        (self.host.as_ref(), self.port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
    }

    pub fn socket_addr_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl ConfigBuilder {
    pub fn nickname<S>(self, nickname: S) -> Self
    where
        S: Into<String>,
    {
        let nickname = nickname.into();

        if nickname.is_empty() {
            return ConfigBuilder(Err(
                ErrorKind::Config("nickname".into(), "is empty".into())
                    .into(),
            ));
        }

        ConfigBuilder(self.0.map(|cfg| Config { nickname, ..cfg }))
    }

    pub fn username<S>(self, username: S) -> Self
    where
        S: Into<String>,
    {
        ConfigBuilder(self.0.map(|cfg| {
            Config {
                username: username.into(),
                ..cfg
            }
        }))
    }

    pub fn realname<S>(self, realname: S) -> Self
    where
        S: Into<String>,
    {
        ConfigBuilder(self.0.map(|cfg| {
            Config {
                realname: realname.into(),
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

impl<'a> IntoConfig for &'a str {
    fn into_config(self) -> Result<Config> {
        read_config(self)
    }
}

impl IntoConfig for String {
    fn into_config(self) -> Result<Config> {
        read_config(&self)
    }
}

impl<R> IntoConfig for BufReader<R>
where
    R: Read,
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

fn read_config(input: &str) -> Result<Config> {
    let mut cfg: Config = serde_yaml::from_str(input)?;

    ensure!(
        !cfg.nickname.is_empty(),
        ErrorKind::Config("nickname".into(), "is empty".into())
    );

    if cfg.username.is_empty() {
        cfg.username = cfg.nickname.clone();
    }

    if cfg.realname.is_empty() {
        cfg.realname = format!(
            "Built with <{}> v{}",
            env!("CARGO_PKG_HOMEPAGE"),
            env!("CARGO_PKG_VERSION")
        );
    }

    ensure!(
        !cfg.servers.is_empty(),
        ErrorKind::Config("servers".into(), "is empty".into())
    );

    ensure!(
        cfg.servers.len() == 1,
        ErrorKind::Config(
            "servers".into(),
            "lists multiple servers, which is not yet supported".into(),
        )
    );

    Ok(cfg)
}

fn mk_true() -> bool {
    true
}
