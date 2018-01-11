pub use self::bot_cmd::BotCmdAttr;
pub use self::bot_cmd::BotCmdAuthLvl;
pub use self::bot_cmd::BotCmdResult;
pub use self::bot_cmd::BotCommand;
pub use self::config::Config;
pub use self::config::IntoConfig;
pub use self::err::Error;
pub use self::err::ErrorKind;
pub use self::err::Result;
pub use self::handler::BotCmdHandler;
pub use self::handler::ErrorHandler;
pub use self::irc_msgs::MsgMetadata;
pub use self::irc_msgs::MsgPrefix;
pub use self::irc_msgs::MsgTarget;
use self::irc_msgs::OwningMsgPrefix;
use self::irc_msgs::parse_msg_to_nick;
use self::irc_send::push_to_outbox;
use self::misc_traits::GetDebugInfo;
pub use self::modl_sys::Module;
use self::modl_sys::ModuleFeatureInfo;
use self::modl_sys::ModuleFeatureKind;
use self::modl_sys::ModuleInfo;
use self::modl_sys::ModuleLoadMode;
pub use self::modl_sys::mk_module;
pub use self::reaction::ErrorReaction;
use self::reaction::LibReaction;
pub use self::reaction::Reaction;
use crossbeam_channel;
use crossbeam_utils;
use irc::client::prelude as aatxe;
use irc::client::server::Server as AatxeServer;
use irc::client::server::utils::ServerExt as AatxeServerExt;
use irc::proto::Message;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::panic::RefUnwindSafe;
use std::panic::UnwindSafe;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use uuid::Uuid;

pub(crate) mod bot_cmd;

mod config;
mod err;
mod handler;
mod irc_comm;
mod irc_msgs;
mod irc_send;
mod misc_traits;
mod modl_sys;
mod reaction;
mod state;

const THREAD_NAME_FAIL: &str = "This thread is unnamed?! We specifically gave it a name; what \
                                happened?!";

const LOCK_EARLY_POISON_FAIL: &str =
    "A lock was poisoned?! Already?! We really oughtn't have panicked yet, so let's panic some \
     more....";

pub struct State {
    config: config::inner::Config,
    servers: BTreeMap<ServerId, RwLock<Server>>,
    addressee_suffix: Cow<'static, str>,
    modules: BTreeMap<Cow<'static, str>, Arc<Module>>,
    commands: BTreeMap<Cow<'static, str>, BotCommand>,
    // TODO: This is server-specific.
    msg_prefix: RwLock<OwningMsgPrefix>,
    error_handler: Arc<ErrorHandler>,
}

// TODO: Split out `inner` struct-of-arrays-style, for the benefits to `irc_send`.
struct Server {
    id: ServerId,
    inner: aatxe::IrcServer,
    config: config::Server,
    socket_addr_string: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct ServerId {
    uuid: Uuid,
}

impl ServerId {
    fn new() -> Self {
        ServerId { uuid: Uuid::new_v4() }
    }
}

impl State {
    fn new<ErrF>(config: config::inner::Config, error_handler: ErrF) -> State
    where
        ErrF: ErrorHandler,
    {
        let msg_prefix = RwLock::new(OwningMsgPrefix::from_string(
            format!("{}!{}@", config.nickname, config.username),
        ));

        State {
            config: config,
            servers: Default::default(),
            addressee_suffix: ": ".into(),
            modules: Default::default(),
            commands: Default::default(),
            msg_prefix,
            error_handler: Arc::new(error_handler),
        }
    }

    fn handle_err<S>(&self, err: Error, desc: S) -> Option<LibReaction<Message>>
    where
        S: Borrow<str>,
    {
        let desc = desc.borrow();

        let reaction = self.error_handler.run(err);

        match reaction {
            ErrorReaction::Proceed => {
                trace!(
                    "Proceeding despite error{}{}{}.",
                    if desc.is_empty() { "" } else { " (" },
                    desc,
                    if desc.is_empty() { "" } else { ")" }
                );
                None
            }
            ErrorReaction::Quit(msg) => {
                trace!(
                    "Quitting because of error{}{}{}.",
                    if desc.is_empty() { "" } else { " (" },
                    desc,
                    if desc.is_empty() { "" } else { ")" }
                );
                Some(irc_comm::mk_quit(msg))
            }
        }
    }

    fn handle_err_generic(&self, err: Error) -> Option<LibReaction<Message>> {
        self.handle_err(err, "")
    }
}

pub fn run<Cfg, ErrF, ModlCtor, Modls>(config: Cfg, error_handler: ErrF, modules: Modls)
where
    Cfg: IntoConfig,
    ErrF: ErrorHandler,
    Modls: IntoIterator<Item = ModlCtor>,
    ModlCtor: Fn() -> Module,
{
    let config = match config.into_config() {
        Ok(c) => {
            trace!("Loaded configuration: {:#?}", c);
            c.inner
        }
        Err(e) => {
            error_handler.run(e);
            error!("Terminal error: Failed to load configuration.");
            return;
        }
    };

    let mut state = State::new(config, error_handler);

    match state.load_modules(modules.into_iter().map(|f| f()), ModuleLoadMode::Add) {
        Ok(()) => {
            trace!("Loaded all requested modules without error.")
        }
        Err(errs) => {
            for err in errs {
                match state.error_handler.run(err) {
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

    let mut servers = BTreeMap::new();

    for server_config in &state.config.servers {
        let aatxe_config = aatxe::Config {
            nickname: Some(state.config.nickname.to_owned()),
            username: Some(state.config.username.to_owned()),
            realname: Some(state.config.realname.to_owned()),
            server: Some(server_config.host.clone()),
            port: Some(server_config.port),
            use_ssl: Some(server_config.tls),
            ..Default::default()
        };

        let aatxe_server = match aatxe::IrcServer::from_config(aatxe_config) {
            Ok(s) => {
                trace!("Connected to server {:?}.", server_config.host);
                s
            }
            Err(err) => {
                match state.error_handler.run(err.into()) {
                    ErrorReaction::Proceed => {
                        error!(
                            "Failed to connect to server {:?}; ignoring.",
                            server_config.host
                        );
                        continue;
                    }
                    ErrorReaction::Quit(msg) => {
                        error!(
                            "Terminal error while connecting to server {:?}: {:?}",
                            server_config.host,
                            msg.unwrap_or_default().as_ref()
                        );
                        return;
                    }
                }
            }
        };

        let server_id = ServerId::new();

        let server = Server {
            id: server_id,
            inner: aatxe_server,
            config: server_config.clone(),
            socket_addr_string: server_config.socket_addr_string(),
        };

        match servers.insert(server_id, RwLock::new(server)) {
            None => {}
            Some(_other_server) => {
                // TODO: If <https://github.com/aatxe/irc/issues/104> is resolved in favor of
                // `IrcServer` implementing `Debug`, add the other server to this message.
                error!(
                    "This shouldn't happen, but there was already a server registered with UUID \
                     {uuid}!",
                    uuid = server_id.uuid.hyphenated(),
                );
                return;
            }
        }
    }

    state.servers = servers;

    let state = Arc::new(state);
    let state = &state;

    crossbeam_utils::scoped::scope(|crossbeam_scope| {
        let (outbox_sender, outbox_receiver) = crossbeam_channel::bounded(irc_send::OUTBOX_SIZE);

        spawn_thread(
            crossbeam_scope,
            state,
            "*".into(),
            "send",
            |_| "sending thread".into(),
            || irc_send::send_main(state.clone(), outbox_receiver),
        );

        for (&server_id, server) in &state.servers {

            let (aatxe_server, addr) = {
                let s = server.read().expect(LOCK_EARLY_POISON_FAIL);
                (s.inner.clone(), s.socket_addr_string.clone())
            };

            let outbox_sender_clone = outbox_sender.clone();

            let recv_fn = move || {
                let current_thread = thread::current();
                let thread_label = current_thread.name().expect(THREAD_NAME_FAIL);

                match aatxe_server.identify() {
                    Ok(()) => debug!("{}: Sent identification sequence to server.", thread_label),
                    Err(e) => {
                        error!(
                            "{}: Failed to send identification sequence to server: {}",
                            thread_label,
                            e
                        )
                    }
                }

                crossbeam_utils::scoped::scope(|crossbeam_scope| {
                    aatxe_server
                        .for_each_incoming(|msg| {
                            handle_msg(
                                state,
                                crossbeam_scope,
                                server_id,
                                &outbox_sender_clone,
                                Ok(msg),
                            )
                        })
                        .map_err(Into::into)
                })
            };

            spawn_thread(
                crossbeam_scope,
                state,
                addr,
                "recv",
                |addr| format!("receiving thread for server {addr:?}", addr = addr),
                recv_fn,
            );
        }
    })
}

fn handle_msg<'xbs, 'xbsr>(
    state: &Arc<State>,
    crossbeam_scope: &'xbsr crossbeam_utils::scoped::Scope<'xbs>,
    server_id: ServerId,
    outbox: &irc_send::OutboxPort,
    input: Result<Message>,
) where
    'xbs: 'xbsr,
{
    match input.and_then(|msg| {
        irc_comm::handle_msg(&state, crossbeam_scope, server_id, outbox, msg)
    }) {
        Ok(()) => {}
        Err(e) => push_to_outbox(outbox, server_id, state.handle_err_generic(e)),
    }
}

fn spawn_thread<'xbs, F, PurposeF>(
    crossbeam_scope: &crossbeam_utils::scoped::Scope<'xbs>,
    state: &Arc<State>,
    addr: String,
    purpose_desc_abbr: &str,
    purpose_desc_full: PurposeF,
    business: F,
) where
    F: FnOnce() -> Result<()> + Send + 'xbs,
    PurposeF: FnOnce(&str) -> String,
{
    let label = format!("{}[{}]", purpose_desc_abbr, addr);

    let thread_build_result = crossbeam_scope.builder().name(label).spawn(move || {
        let current_thread = thread::current();
        let thread_label = current_thread.name().expect(THREAD_NAME_FAIL);

        match business() {
            Ok(()) => debug!("{}: Thread exited successfully.", thread_label),
            Err(err) => error!("{}: Thread exited with error: {:?}", thread_label, err),
        }
    });

    match thread_build_result {
        Ok(_join_handle) => {}
        Err(err) => {
            match state.error_handler.run(err.into()) {
                ErrorReaction::Proceed => {
                    error!(
                        "Failed to create {purpose}; ignoring.",
                        purpose = purpose_desc_full(&addr),
                    )
                }
                ErrorReaction::Quit(msg) => {
                    error!(
                        "Terminal error: Failed to create {purpose}: {msg:?}",
                        purpose = purpose_desc_full(&addr),
                        msg = msg
                    )
                }
            }
        }
    }
}
