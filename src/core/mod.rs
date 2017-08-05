pub use self::bot_cmd::BotCmdAuthLvl;
pub use self::bot_cmd::BotCmdResult;
pub use self::bot_cmd::BotCommand;
pub use self::bot_cmd_handler::BotCmdHandler;
pub use self::err::Error;
pub use self::err::ErrorKind;
pub use self::err::Result;
pub use self::irc_msgs::MsgMetadata;
pub use self::irc_msgs::MsgPrefix;
pub use self::irc_msgs::MsgTarget;
use self::irc_msgs::OwningMsgPrefix;
use self::irc_msgs::parse_msg_to_nick;
use self::misc_traits::GetDebugInfo;
pub use self::modl_sys::Module;
use self::modl_sys::ModuleFeatureInfo;
use self::modl_sys::ModuleFeatureKind;
use self::modl_sys::ModuleInfo;
use self::modl_sys::ModuleLoadMode;
pub use self::modl_sys::mk_module;
pub use self::reaction::ErrorReaction;
pub use self::reaction::Reaction;
use yak_irc::client::Client;
use yak_irc::client::Reaction as LibReaction;
use yak_irc::client::prelude::*;
use parking_lot::Mutex;
use parking_lot::RwLock;
use pircolate::Message;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;

mod bot_cmd;
mod bot_cmd_handler;
mod config;
mod err;
mod irc_comm;
mod irc_msgs;
mod irc_send;
mod misc_traits;
mod modl_sys;
mod reaction;
mod state;

pub struct State<'server, 'modl> {
    _lifetime_server: PhantomData<&'server ()>,
    config: Config,
    addressee_suffix: Cow<'static, str>,
    chars_indicating_msg_is_addressed_to_nick: Vec<char>,
    modules: BTreeMap<Cow<'static, str>, &'modl Module<'modl>>,
    commands: BTreeMap<Cow<'static, str>, BotCommand<'modl>>,
    msg_prefix: RwLock<OwningMsgPrefix>,
    error_handler: Arc<Fn(Error) -> ErrorReaction + Send + Sync>,
}

// TODO: once pub_restricted hits stable (1.18), move this into the `config` module.
#[derive(Debug)]
pub struct Config {
    nick: String,
    username: String,
    realname: String,
    admins: Vec<config::Admin>,
    servers: Vec<config::Server>,
    channels: Vec<String>,
}

impl<'server, 'modl> State<'server, 'modl> {
    fn new<ErrF>(config: Config, error_handler: ErrF) -> State<'server, 'modl>
    where
        ErrF: 'static + Fn(Error) -> ErrorReaction + Send + Sync,
    {
        let nick = config.nick.clone();
        let username = config.username.clone();

        State {
            _lifetime_server: PhantomData,
            config: config,
            addressee_suffix: ": ".into(),
            chars_indicating_msg_is_addressed_to_nick: vec![':', ','],
            modules: Default::default(),
            commands: Default::default(),
            msg_prefix: RwLock::new(OwningMsgPrefix::from_string(
                format!("{}!{}@", nick, username),
            )),
            error_handler: Arc::new(error_handler),
        }
    }

    fn handle_err<E, S>(&self, err: E, desc: S) -> LibReaction<Message>
    where
        E: Into<Error>,
        S: Borrow<str>,
    {
        let desc = desc.borrow();

        let reaction = match err.into() {
            Error(ErrorKind::ModuleRequestedQuit(msg), _) => ErrorReaction::Quit(msg),
            e => (self.error_handler)(e),
        };

        match reaction {
            ErrorReaction::Proceed => {
                trace!(
                    "Proceeding despite error{}{}{}.",
                    if desc.is_empty() { "" } else { " (" },
                    desc,
                    if desc.is_empty() { "" } else { ")" }
                );
                LibReaction::None
            }
            ErrorReaction::Quit(msg) => {
                trace!(
                    "Quitting because of error{}{}{}.",
                    if desc.is_empty() { "" } else { " (" },
                    desc,
                    if desc.is_empty() { "" } else { ")" }
                );
                irc_comm::quit(self, msg)
            }
        }
    }

    fn handle_err_generic<E>(&self, err: E) -> LibReaction<Message>
    where
        E: Into<Error>,
    {
        self.handle_err(err, "")
    }
}

pub fn run<'modl, Cfg, ErrF, Modls>(config: Cfg, error_handler: ErrF, modules: Modls)
where
    Cfg: config::IntoConfig,
    ErrF: 'static + Fn(Error) -> ErrorReaction + Send + Sync,
    Modls: AsRef<[Module<'modl>]>,
{
    let config = match config.into_config() {
        Ok(c) => {
            trace!("Loaded configuration: {:#?}", c);
            c
        }
        Err(e) => {
            error_handler(e.into());
            error!("Terminal error: Failed to load configuration.");
            return;
        }
    };


    let client = {
        let ref server = config.servers[0];

        let mut cli = Client::new();

        let connection = match PlaintextConnection::from_addr(server.resolve()) {
            Ok(conn) => conn,
            Err(err) => {
                error_handler(err.into());
                error!(
                    "Terminal error: Failed to connect to server {}.",
                    server.socket_addr_string()
                );
                return;
            }
        };

        match session::build()
            .nickname(config.nick.clone())
            .username(config.username.clone())
            .realname(config.realname.clone())
            .connection(connection)
            .start() {
            Ok(session) => cli.add_session(session).unwrap(),
            Err(err) => {
                // TODO
                unimplemented!()
            }
        };

        cli
    };

    let mut state = State::new(config, error_handler);

    match state.load_modules(modules.as_ref().iter(), ModuleLoadMode::Add) {
        Ok(()) => {
            trace!("Loaded all requested modules without error.")
        }
        Err(errs) => {
            for err in errs {
                match (state.error_handler)(err) {
                    ErrorReaction::Proceed => {}
                    ErrorReaction::Quit(msg) => {
                        error!(
                            "Terminal error while loading modules: {:?}",
                            msg.unwrap_or_default().as_ref()
                        );
                        return;
                    }
                }
            }
        }
    }

    info!(
        "Loaded modules: {:?}",
        state.modules.keys().collect::<Vec<_>>()
    );
    info!(
        "Loaded commands: {:?}",
        state.commands.keys().collect::<Vec<_>>()
    );

    trace!("Running bot....");

    client
        .run(|ctx, msg| handle_msg(&state, msg.map_err(Into::into)))
        .unwrap()
}

fn handle_msg(state: &State, input: Result<Message>) -> LibReaction<Message> {
    match input.and_then(|msg| irc_comm::handle_msg(&state, msg)) {
        Ok(r) => r,
        Err(e) => state.handle_err_generic(e),
    }
}
