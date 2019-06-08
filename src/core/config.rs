use super::aatxe;
use super::pkg_info;
use super::ErrorKind;
use super::Result;
use serde_yaml;
use smallvec::SmallVec;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use util::irc::ChannelName;
use util::lock::RoLock;
use util::regex::config as rx_cfg;
use util::regex::Regex;

mod inner {
    use smallvec::SmallVec;

    /// Configuration structure that can be deserialized by Serde.
    ///
    /// This is hidden from the consumer because Serde won't validate the configuration.
    #[derive(Debug, Default, Deserialize)]
    pub(super) struct Config {
        pub(super) nickname: String,

        #[serde(default)]
        pub(super) username: String,

        #[serde(default)]
        pub(super) realname: String,

        // TODO: admins should be per-server.
        #[serde(default)]
        pub(super) admins: SmallVec<[super::Admin; 8]>,

        pub(super) servers: SmallVec<[super::Server; 8]>,
    }
}

/// Configuration for IRC bots
///
/// An IRC bot made with this library may (indeed, for any normal usage, must) be configured,
/// specifying such things as the bot's IRC nickname and the server to which it should connect.
///
/// # Configuring a bot in YAML
///
/// To configure the bot using a [YAML] configuration file, create such a file and then use
/// [`Config::try_from_path`] to read and parse it into a [`Config`] structure.
///
/// The text of the configuration file should constitute a YAML mapping with the key-value pairs
/// (hereinafter termed _fields_) that follow, listed by their keys:
///
/// - `nickname` — The value of this field should be a string, which is to be used as the bot's
/// default IRC nickname.
///
/// - `username` — The value of this field, if specified, should be a string, which is to be used
/// as the bot's IRC username (which has little effect in most cases). This field is optional; its
/// value defaults to the given `nickname`.
///
/// - `realname` — The value of this field, if specified, should be a string, which is to be used
/// as the bot's IRC "realname" or "GECOS string", which has even less effect than the username and
/// often is used to display information about a bot's software. This field is optional; its value
/// defaults to information about the bot's software.
///
/// - `servers` — The value of this field should be a sequence of mappings, which specify IRC
/// servers to which the bot should attempt to connect. The fields of these mappings are termed
/// _per-server settings_ and are documented below.
///
///   The available per-server settings for each server follow, listed by their keys:
///
///   - `name` — The value of this field should be a string that does not include a US-ASCII
///   character considered a Common Separator in Unicode (namely `,`, `.`, `/`, or `:`). This field
///   specifies a name to be used to identify the server.
///
///     A concept that depends on this field is the **_channel identifier_**. For each server, each
///     IRC channel thereon that is known to the bot is assigned a channel identifier, which is a
///     string that is the concatenation of—
///
///     - the server's `name`;
///
///     - a slash character (`/`); and
///
///     - the name of the channel, including any leading `#` character.
///
///     An example of a channel identifier is `freenode/#botters-test`.
///
///   - `host` — The value of this field should be a string specifying the hostname of the server,
///   such as `"chat.freenode.net"`.
///
///   - `port` — The value of this field should be a non-negative integer specifying the number of
///   the TCP port at which the server serves IRC, such as `6697`.
///
///   - `nick password` — The value of this field, if specified, should be a string specifying a
///   password to be used to verify that the bot is authorized to use the nickname that has been
///   specified, e.g., a NickServ password. This field is optional.
///
///   - `server password` — The value of this field, if specified, should be a string specifying a
///   password to be used to verify that the bot is authorized to connect to the server, i.e., a
///   password to be sent with the IRC protocol command `PASS` at the start of the IRC session.
///
///   - `TLS` — The value of this field, if specified, should be `true` or `false`, specifying
///   whether the bot should attempt to connect to the server using Transport Layer Security (TLS).
///   This field is optional; its value defaults to `true`.
///
///   - `channels` — The value of this field should be a sequence of mappings, which specify IRC
///   channels on the server. The fields of these mappings are termed _per-channel settings_ and
///   will be documented after the following code example.
///
///     ```yaml
///     servers:
///     - name: freenode
///       host: chat.freenode.net
///       port: 7070
///     - name: Mozilla
///       host: irc.mozilla.org
///       port: 6697
///       channels:
///         - name: '#rust'
///           can see: 'freenode/##rust'
///           seen by: 'Mozilla/#rust-.*'
///         - name: '#rust-offtopic'
///           seen by: 'Mozilla/#rust-.*'
///     - name: other server
///       host: irc.example.net
///       channels:
///         - name: '#scryers'
///           can see: '.*'
///     ```
///
///     In the above example, per-channel settings are specified such that—
///
///     - the Mozilla channel `#rust` can see the freenode channel `##rust`,
///
///     - all Mozilla channels whose names begin with `#rust-` can see `#rust`,
///
///     - all Mozilla channels whose names begin with `#rust-` can see `#rust-offtopic`, and
///
///     - the fictitious channel `#scryers` can see all channels.
///
///     In the documentation for this field, a channel `L` being able to **_see_** a channel `Q`
///     means that the bot _should_ be willing to display data (e.g., quotations) from `Q` (1) in
///     `L` and (2) in one-to-one messaging with any user who, the bot believes, is present in `L`
///     (as of 2018-11-03, the bot is incapable of forming such beliefs, and so should not display
///     quotations in one-to-one messaging). All channels can see themselves. By default, the bot
///     _should_ refuse to display data from a channel `Q` (1) in all channels that cannot see `Q`
///     and (2) in one-to-one messaging with all users who, the bot believes, are not present in
///     any channel that can see `Q`.
///
///     These restrictions on which channels can see which other channels are subject to the
///     following limitations:
///
///     - Administrators of the bot may be permitted to override these restriction if they so
///     choose.
///
///     - The word "should" is emphasized in the definition of "see" above because one relies on
///     individual bot modules to implement such restrictions based on these settings. All bot
///     modules provided with this library should implement these restrictions; please file bug
///     reports if they do not.
///
///     The available per-channel settings for each channel `C` follow, listed by their keys:
///
///     - `name` — The value of this per-channel setting should be a string, specifying the name of
///     the channel `C`, such as `##rust`.
///
///     - `autojoin` — The value of this per-channel setting should be `true` or `false`,
///     specifying whether the bot should attempt to join the channel `C` upon connecting to the
///     server. This field is optional; its value defaults to `true`. TODO: This remains to be
///     implemented.
///
///     - `can see` — The value of this per-channel setting should be a string, which will be
///     parsed as a regular expression using the Rust [`regex`] library and [its particular
///     syntax][`regex` syntax]. The channel `C` will be able to see all channels whose identifiers
///     match this regular expression. This regular expression is _anchored_; i.e., it will be
///     considered to match a channel identifier only if it matches the whole of the channel
///     identifier rather than only part of it; e.g., the anchored regular expression
///     `Mozilla/#rust` will match the channel identifier `Mozilla/#rust` but not the channel
///     identifier `Mozilla/#rust-offtopic`, which, in contrast, it would match were it not
///     anchored.
///
///     - `seen by`: The value of this per-channel setting should be a string, which will be parsed
///     as an anchored Rust regular expression in the same way as the value of the per-channel
///     setting with the key `can see`. All channels whose identifiers match this regular
///     expression will be able to see the channel `C`.
///
///
/// [YAML]: <https://en.wikipedia.org/wiki/YAML>
/// [`Config::try_from_path`]: <struct.Config.html#method.try_from_path>
/// [`Config`]: <struct.Config.html>
/// [`regex` flag]: <https://docs.rs/regex/*/regex/#grouping-and-flags>
/// [`regex` syntax]: <https://docs.rs/regex/*/regex/#syntax>
/// [`regex`]: <https://docs.rs/regex/*/regex/>
#[derive(Debug)]
pub struct Config {
    pub(super) nickname: String,

    pub(super) username: String,

    pub(super) realname: String,

    pub(super) admins: SmallVec<[Admin; 8]>,

    pub(super) servers: SmallVec<[Server; 8]>,

    pub(super) aatxe_configs: SmallVec<[Arc<aatxe::Config>; 8]>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Admin {
    #[serde(default)]
    pub nick: Option<String>,

    #[serde(default)]
    pub user: Option<String>,

    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Server {
    // TODO: Use a `ServerName` newtype that checks that the string is a valid identifier.
    pub name: String,

    pub host: String,

    pub port: u16,

    #[serde(rename = "nick password")]
    pub(super) nick_password: Option<String>,

    #[serde(rename = "server password")]
    pub(super) server_password: Option<String>,

    #[serde(default = "mk_true", rename = "TLS")]
    pub tls: bool,

    #[serde(default)]
    pub channels: SmallVec<[Channel; 24]>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Channel {
    pub name: ChannelName,

    #[serde(rename = "can see")]
    pub can_see: Option<RoLock<Regex<rx_cfg::Anchored>>>,

    #[serde(rename = "seen by")]
    pub seen_by: Option<RoLock<Regex<rx_cfg::Anchored>>>,
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
        ConfigBuilder(Ok(Default::default()))
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
                ErrorKind::Config("nickname".into(), "is empty".into()).into()
            ));
        }

        ConfigBuilder(self.0.map(|cfg| inner::Config { nickname, ..cfg }))
    }

    pub fn username<S>(self, username: S) -> Self
    where
        S: Into<String>,
    {
        ConfigBuilder(self.0.map(|cfg| inner::Config {
            username: username.into(),
            ..cfg
        }))
    }

    pub fn realname<S>(self, realname: S) -> Self
    where
        S: Into<String>,
    {
        ConfigBuilder(self.0.map(|cfg| inner::Config {
            realname: realname.into(),
            ..cfg
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
    serde_yaml::from_str(input)
        .map_err(Into::into)
        .and_then(cook_config)
}

fn cook_config(mut cfg: inner::Config) -> Result<Config> {
    validate_config(&cfg)?;

    fill_in_config_defaults(&mut cfg)?;

    let inner::Config {
        nickname,
        username,
        realname,
        admins,
        servers,
    } = cfg;

    let aatxe_configs = servers
        .iter()
        .map(|server_cfg| {
            let &Server {
                name: _,
                ref host,
                port,
                tls,
                ref nick_password,
                ref server_password,
                ref channels,
            } = server_cfg;

            Arc::new(aatxe::Config {
                // TODO: Allow nickname etc. to be configured per-server.
                nickname: Some(nickname.clone()),
                nick_password: nick_password.clone(),
                password: server_password.clone(),
                username: Some(username.clone()),
                realname: Some(realname.clone()),
                server: Some(host.clone()),
                port: Some(port),
                use_ssl: Some(tls),
                channels: Some(
                    channels
                        .iter()
                        .map(|chan| chan.name.as_ref().into())
                        .collect(),
                ),
                ..Default::default()
            })
        }).collect();

    Ok(Config {
        nickname,
        username,
        realname,
        admins,
        servers,
        aatxe_configs,
    })
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
