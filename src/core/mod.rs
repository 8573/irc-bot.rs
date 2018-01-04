pub use self::bot_cmd::BotCmdAuthLvl;
pub use self::bot_cmd::BotCmdResult;
pub use self::bot_cmd::BotCommand;
pub use self::bot_cmd_handler::BotCmdHandler;
pub use self::config::Config;
pub use self::config::IntoConfig;
pub use self::err::Error;
pub use self::err::ErrorKind;
pub use self::err::Result;
pub use self::irc_msgs::MsgMetadata;
pub use self::irc_msgs::MsgPrefix;
pub use self::irc_msgs::MsgTarget;
use self::irc_msgs::OwningMsgPrefix;
use self::irc_msgs::parse_msg_to_nick;
use self::irc_send::OutboxRecord;
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
use crossbeam;
use irc::client::prelude as aatxe;
use irc::client::server::Server as AatxeServer;
use irc::client::server::utils::ServerExt as AatxeServerExt;
use irc::proto::Message;
use parking_lot::Mutex;
use parking_lot::RwLock;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

pub(crate) mod bot_cmd;

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

pub struct State {
    config: config::inner::Config,
    servers: Vec<Server>,
    addressee_suffix: Cow<'static, str>,
    chars_indicating_msg_is_addressed_to_nick: Vec<char>,
    modules: BTreeMap<Cow<'static, str>, Arc<Module>>,
    commands: BTreeMap<Cow<'static, str>, BotCommand>,
    msg_prefix: RwLock<OwningMsgPrefix>,
    error_handler: Arc<Fn(&Error) -> ErrorReaction + Send + Sync>,
}

#[derive(Clone)]
struct Server {
    inner: aatxe::IrcServer,
    config: config::Server,
}

impl State {
    fn new<ErrF>(config: config::inner::Config, error_handler: ErrF) -> State
    where
        ErrF: 'static + Fn(&Error) -> ErrorReaction + Send + Sync,
    {
        let msg_prefix = RwLock::new(OwningMsgPrefix::from_string(
            format!("{}!{}@", config.nickname, config.username),
        ));

        State {
            config: config,
            servers: Default::default(),
            addressee_suffix: ": ".into(),
            chars_indicating_msg_is_addressed_to_nick: vec![':', ','],
            modules: Default::default(),
            commands: Default::default(),
            msg_prefix,
            error_handler: Arc::new(error_handler),
        }
    }

    fn handle_err<S>(&self, err: &Error, desc: S) -> LibReaction<Message>
    where
        S: Borrow<str>,
    {
        let desc = desc.borrow();

        let reaction = match err {
            &Error(ErrorKind::ModuleRequestedQuit(ref msg), _) => ErrorReaction::Quit(msg.clone()),
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

    fn handle_err_generic(&self, err: &Error) -> LibReaction<Message> {
        self.handle_err(err, "")
    }
}

pub fn run<Cfg, ErrF, ModlCtor, Modls>(config: Cfg, error_handler: ErrF, modules: Modls)
where
    Cfg: IntoConfig,
    ErrF: 'static + Fn(&Error) -> ErrorReaction + Send + Sync,
    Modls: IntoIterator<Item = ModlCtor>,
    ModlCtor: Fn() -> Module,
{
    let config = match config.into_config() {
        Ok(c) => {
            trace!("Loaded configuration: {:#?}", c);
            c.inner
        }
        Err(e) => {
            error_handler(&e);
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
                match (state.error_handler)(&err) {
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

    let mut servers = Vec::new();

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
                match (state.error_handler)(&err.into()) {
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

        servers.push(Server {
            inner: aatxe_server,
            config: server_config.clone(),
        });
    }

    state.servers = servers;

    let state = Arc::new(state);

    run_server_threads(
        &state,
        |state, Server { inner: server, .. }, label, outbox| {
            match server.identify() {
                Ok(()) => debug!("{}: Sent identification sequence to server.", label),
                Err(err) => {
                    error!(
                        "{}: Failed to send identification sequence to server.",
                        label
                    )
                }
            }

            server
                .for_each_incoming(|msg| handle_msg(&state, &outbox, label, Ok(msg)))
                .map_err(Into::into)
        },
        |state, server, label, outbox| irc_send::send_main(state, server, label, outbox),
    );
}

fn handle_msg(
    state: &State,
    outbox: &irc_send::OutboxPort,
    thread_label: &str,
    input: Result<Message>,
) {
    let reaction = match input {
        Ok(ref msg) => {
            match irc_comm::handle_msg(&state, msg.clone()) {
                Ok(r) => r,
                Err(e) => state.handle_err_generic(&e),
            }
        }
        Err(ref e) => state.handle_err_generic(e),
    };

    push_to_outbox(outbox, thread_label, input, reaction);
}

fn run_server_threads<RF, SF>(state: &Arc<State>, recv_fn: RF, send_fn: SF)
where
    RF: Fn(Arc<State>, Server, &str, mpsc::SyncSender<OutboxRecord>) -> Result<()> + Send + Sync,
    SF: Fn(Arc<State>, Server, &str, mpsc::Receiver<OutboxRecord>) -> Result<()> + Send + Sync,
{
    crossbeam::scope(|scope| {
        let mut join_handles = Vec::<crossbeam::ScopedJoinHandle<()>>::new();

        for server in &state.servers {
            let (outbox_sender, outbox_receiver) = mpsc::sync_channel(irc_send::OUTBOX_SIZE);

            spawn_server_thread(
                scope,
                &mut join_handles,
                state,
                server,
                "send",
                "sending",
                &send_fn,
                outbox_receiver,
            );

            spawn_server_thread(
                scope,
                &mut join_handles,
                state,
                server,
                "recv",
                "receiving",
                &recv_fn,
                outbox_sender,
            );
        }
    })
}

fn spawn_server_thread<'s, F, OutboxPort>(
    scope: &crossbeam::Scope<'s>,
    join_handles: &mut Vec<crossbeam::ScopedJoinHandle<()>>,
    state: &Arc<State>,
    server: &Server,
    purpose_desc_abbr: &str,
    purpose_desc_full: &str,
    business: F,
    outbox_port: OutboxPort,
) where
    F: FnOnce(Arc<State>, Server, &str, OutboxPort) -> Result<()> + Send + 's,
    OutboxPort: Send + 's,
{
    let state_handle = state.clone();
    let server_clone = server.clone();
    let addr = server.config.socket_addr_string();
    let label = format!("{}[{}]", purpose_desc_abbr, addr);

    let thread_build_result = scope.builder().name(label).spawn(move || {
        let current_thread = thread::current();
        let label = current_thread.name().expect(
            "This thread is unnamed?! We specifically gave it a \
             name; what happened?!",
        );

        match business(state_handle, server_clone, label, outbox_port) {
            Ok(()) => debug!("{}: Thread exited successfully.", label),
            Err(err) => error!("{}: Thread exited with error: {:?}", label, err),
        }
    });

    match thread_build_result {
        Ok(join_handle) => join_handles.push(join_handle),
        Err(err) => {
            match (state.error_handler)(&err.into()) {
                ErrorReaction::Proceed => {
                    error!(
                        "Failed to create {purpose} thread for server {addr:?}; ignoring.",
                        purpose = purpose_desc_full,
                        addr = addr
                    )
                }
                ErrorReaction::Quit(msg) => {
                    error!(
                        "Terminal error: Failed to create {purpose} thread for server {addr:?}: \
                         {msg:?}",
                        purpose = purpose_desc_full,
                        addr = addr,
                        msg = msg
                    )
                }
            }
        }
    }
}
