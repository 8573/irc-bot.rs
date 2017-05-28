pub use self::bot_cmd::BotCmdAuthLvl;
pub use self::bot_cmd::BotCmdResult;
pub use self::bot_cmd::BotCommand;
pub use self::bot_cmd_handler::BotCmdHandler;
pub use self::config::Config;
pub use self::err::Error;
pub use self::err::ErrorKind;
pub use self::err::Result;
pub use self::irc_msgs::MsgMetadata;
pub use self::irc_msgs::MsgPrefix;
pub use self::irc_msgs::MsgTarget;
use self::irc_msgs::parse_msg_to_nick;
use self::irc_msgs::parse_prefix;
use self::misc_traits::GetDebugInfo;
pub use self::modl_sys::Module;
use self::modl_sys::ModuleFeatureInfo;
use self::modl_sys::ModuleFeatureKind;
use self::modl_sys::ModuleInfo;
use self::modl_sys::ModuleLoadMode;
pub use self::modl_sys::mk_module;
pub use self::reaction::ErrorReaction;
pub use self::reaction::Reaction;
use irc::client::prelude::*;
use std::borrow::Cow;
use std::collections::BTreeMap;

mod bot_cmd;
mod bot_cmd_handler;
mod config;
mod err;
mod irc_comm;
mod irc_msgs;
mod misc_traits;
mod modl_sys;
mod reaction;
mod state;

pub struct State<'server, 'modl> {
    server: &'server IrcServer,
    addressee_suffix: Cow<'static, str>,
    chars_indicating_msg_is_addressed_to_nick: Vec<char>,
    modules: BTreeMap<Cow<'static, str>, &'modl Module<'modl>>,
    commands: BTreeMap<Cow<'static, str>, BotCommand<'modl>>,
    msg_prefix_string: String,
}

impl<'server, 'modl> State<'server, 'modl> {
    fn new(server: &'server IrcServer) -> State<'server, 'modl> {
        State {
            server: server,
            addressee_suffix: ": ".into(),
            chars_indicating_msg_is_addressed_to_nick: vec![':', ','],
            modules: Default::default(),
            commands: Default::default(),
            msg_prefix_string: format!("{}!{}@",
                                       server.current_nickname(),
                                       server
                                           .config()
                                           .username
                                           .as_ref()
                                           .unwrap_or(&String::new())),
        }
    }

    fn run<ErrF>(&mut self, mut error_handler: ErrF)
        where ErrF: FnMut(Error) -> ErrorReaction
    {
        trace!("Running bot....");
        info!("Loaded modules: {:?}",
              self.modules.keys().collect::<Vec<_>>());
        info!("Loaded commands: {:?}",
              self.commands.keys().collect::<Vec<_>>());

        'main_loop: for msg in self.server.iter() {
            match irc_comm::handle_msg(self, msg).map_err(|err| match err {
                                                    Error(ErrorKind::ModuleRequestedQuit(msg),
                                                          _) => ErrorReaction::Quit(msg),
                                                    e => error_handler(e),
                                                }) {
                Ok(()) => {}
                Err(ErrorReaction::Proceed) => {}
                Err(ErrorReaction::Quit(msg)) => {
                    irc_comm::quit(self, msg);
                    break 'main_loop;
                }
            }
        }
    }
}

pub fn run<'modl, Cfg, ErrF, Modls>(config: Cfg, mut error_handler: ErrF, modules: Modls)
    where Cfg: config::IntoConfig,
          ErrF: FnMut(Error) -> ErrorReaction,
          Modls: AsRef<[Module<'modl>]>
{
    let config = config.into_config();

    info!("{:?}", config);

    return;

    let server = match IrcServer::new("") {
        Ok(s) => s,
        Err(e) => {
            error_handler(e.into());
            return;
        }
    };

    match server
              .identify()
              .map_err(|err| ErrorKind::IdentificationFailure(err)) {
        Ok(()) => {}
        Err(e) => {
            error_handler(e.into());
            return;
        }
    };

    let mut state = State::new(&server);

    match state.load_modules(modules.as_ref().iter(), ModuleLoadMode::Add) {
        Ok(()) => {}
        Err(errs) => {
            for err in errs {
                match error_handler(err) {
                    ErrorReaction::Proceed => {}
                    ErrorReaction::Quit(msg) => {
                        error!("Terminal error while loading modules: {:?}",
                               msg.unwrap_or_default().as_ref());
                        return;
                    }
                }
            }
        }
    }

    state.run(error_handler);
}
