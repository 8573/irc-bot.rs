use super::ErrorKind;
use super::Result;
use super::pkg_info;
use serde_yaml;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::path::Path;

pub(crate) mod inner {
    /// Configuration structure that can be deserialized by Serde.
    ///
    /// This is hidden from the consumer because Serde won't validate the configuration.
    #[derive(Debug, Deserialize)]
    pub(crate) struct Config {
        pub(crate) nickname: String,

        #[serde(default)]
        pub(crate) username: String,

        #[serde(default)]
        pub(crate) realname: String,

        #[serde(default)]
        pub(crate) admins: Vec<super::Admin>,

        pub(crate) servers: Vec<super::Server>,
    }
}

pub struct Config {
    pub(crate) inner: inner::Config,
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
pub struct ConfigBuilder(Result<inner::Config>);

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
        ConfigBuilder(Self::try_from(input).map(|cfg| cfg.inner))
    }

    pub fn build_from_path<P>(path: P) -> ConfigBuilder
    where
        P: AsRef<Path>,
    {
        ConfigBuilder(Self::try_from_path(path).map(|cfg| cfg.inner))
    }
}

impl Server {
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

        ConfigBuilder(self.0.map(|cfg| inner::Config { nickname, ..cfg }))
    }

    pub fn username<S>(self, username: S) -> Self
    where
        S: Into<String>,
    {
        ConfigBuilder(self.0.map(|cfg| {
            inner::Config {
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
            inner::Config {
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
        self.0.and_then(cook_config)
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
    serde_yaml::from_str(input).map_err(Into::into).and_then(
        cook_config,
    )
}

fn cook_config(mut cfg: inner::Config) -> Result<Config> {
    validate_config(&cfg)?;

    fill_in_config_defaults(&mut cfg)?;

    Ok(Config { inner: cfg })
}

fn validate_config(cfg: &inner::Config) -> Result<()> {
    ensure!(
        !cfg.nickname.is_empty(),
        ErrorKind::Config("nickname".into(), "is empty".into())
    );

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

    Ok(())
}

fn fill_in_config_defaults(cfg: &mut inner::Config) -> Result<()> {
    if cfg.username.is_empty() {
        cfg.username = cfg.nickname.clone();
    }

    if cfg.realname.is_empty() {
        cfg.realname = pkg_info::BRIEF_CREDITS_STRING.clone();
    }

    Ok(())
}

fn mk_true() -> bool {
    true
}

// Manually implement `Debug` so we get
//
//     Config { .. }
//
// rather than
//
//     Config { inner: Config { .. } }
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
