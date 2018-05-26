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
pub use self::handler::TriggerHandler;
pub use self::irc_msgs::MsgDest;
pub use self::irc_msgs::MsgMetadata;
pub use self::irc_msgs::MsgPrefix;
use self::irc_msgs::OwningMsgPrefix;
use self::irc_msgs::parse_msg_to_nick;
use self::irc_send::push_to_outbox;
use self::misc_traits::GetDebugInfo;
pub use self::modl_sys::Module;
use self::modl_sys::ModuleFeatureInfo;
use self::modl_sys::ModuleInfo;
use self::modl_sys::ModuleLoadMode;
pub use self::modl_sys::mk_module;
pub use self::reaction::ErrorReaction;
use self::reaction::LibReaction;
pub use self::reaction::Reaction;
pub use self::trigger::Trigger;
pub use self::trigger::TriggerAttr;
pub use self::trigger::TriggerPriority;
use crossbeam_channel;
use irc::client::prelude as aatxe;
use irc::client::prelude::ClientExt as AatxeClientExt;
use irc::proto::Message;
use rand::EntropyRng;
use rand::SeedableRng;
use rand::StdRng;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;
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
mod pkg_info;
mod reaction;
mod state;
mod trigger;

const THREAD_NAME_FAIL: &str = "This thread is unnamed?! We specifically gave it a name; what \
                                happened?!";

const LOCK_EARLY_POISON_FAIL: &str =
    "A lock was poisoned?! Already?! We really oughtn't have panicked yet, so let's panic some \
     more....";

pub struct State {
    aatxe_clients: RwLock<BTreeMap<ServerId, aatxe::IrcClient>>,
    addressee_suffix: Cow<'static, str>,
    commands: BTreeMap<Cow<'static, str>, BotCommand>,
    config: config::Config,
    error_handler: Arc<ErrorHandler>,
    module_data_path: PathBuf,
    modules: BTreeMap<Cow<'static, str>, Arc<Module>>,
    // TODO: This is server-specific.
    msg_prefix: RwLock<OwningMsgPrefix>,
    rng: Mutex<StdRng>,
    servers: BTreeMap<ServerId, RwLock<Server>>,
    triggers: BTreeMap<TriggerPriority, Vec<Trigger>>,
}

#[derive(Debug)]
struct Server {
    id: ServerId,
    aatxe_config: Arc<aatxe::Config>,
    socket_addr_string: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct ServerId {
    uuid: Uuid,
}

impl ServerId {
    fn new() -> Self {
        ServerId {
            uuid: Uuid::new_v4(),
        }
    }
}

impl State {
    fn new<ErrF>(
        config: config::Config,
        module_data_path: PathBuf,
        error_handler: ErrF,
    ) -> Result<State>
    where
        ErrF: ErrorHandler,
    {
        let msg_prefix = RwLock::new(OwningMsgPrefix::from_string(format!(
            "{}!{}@",
            config.nickname, config.username
        )));

        Ok(State {
            aatxe_clients: Default::default(),
            addressee_suffix: ": ".into(),
            commands: Default::default(),
            config: config,
            error_handler: Arc::new(error_handler),
            module_data_path,
            modules: Default::default(),
            msg_prefix,
            rng: Mutex::new(StdRng::from_rng(EntropyRng::new())?),
            servers: Default::default(),
            triggers: Default::default(),
        })
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

pub fn run<Cfg, ModlData, ErrF, ModlCtor, Modls>(
    config: Cfg,
    module_data_path: ModlData,
    error_handler: ErrF,
    modules: Modls,
) where
    Cfg: IntoConfig,
    ModlData: Into<PathBuf>,
    ErrF: ErrorHandler,
    Modls: IntoIterator<Item = ModlCtor>,
    ModlCtor: Fn() -> Module,
{
    let config = match config.into_config() {
        Ok(cfg) => {
            trace!("Loaded configuration: {:#?}", cfg);
            cfg
        }
        Err(e) => {
            error_handler.run(e);
            error!("Terminal error: Failed to load configuration.");
            return;
        }
    };

    let mut state = match State::new(config, module_data_path.into(), error_handler) {
        Ok(s) => {
            trace!("Assembled bot state.");
            s
        }
        Err(e) => {
            error!("Terminal error while assembling bot state: {}", e);
            return;
        }
    };

    match state.load_modules(modules.into_iter().map(|f| f()), ModuleLoadMode::Add) {
        Ok(()) => trace!("Loaded all requested modules without error."),
        Err(errs) => for err in errs {
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
        },
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

    for aatxe_config in &state.config.servers {
        let server_id = ServerId::new();

        let socket_addr_string = match (&aatxe_config.server, aatxe_config.port) {
            (Some(h), Some(p)) => format!("{}:{}", h, p),
            (Some(h), None) => format!("{}:<unknown port>", h),
            (None, Some(p)) => format!("<unknown hostname>:{}", p),
            (None, None) => format!("<unknown hostname>:<unknown port>"),
        };

        let server = Server {
            id: server_id,
            aatxe_config: aatxe_config.clone(),
            socket_addr_string,
        };

        match servers.insert(server_id, RwLock::new(server)) {
            None => {}
            Some(other_server) => {
                error!(
                    "This shouldn't happen, but there was already a server registered with UUID \
                     {uuid}: {other_server:?}",
                    uuid = server_id.uuid.hyphenated(),
                    other_server = other_server.read().expect(LOCK_EARLY_POISON_FAIL),
                );
                return;
            }
        }
    }

    state.servers = servers;

    let state = Arc::new(state);
    trace!("Stored bot state onto heap.");

    let (outbox_sender, outbox_receiver) = crossbeam_channel::bounded(irc_send::OUTBOX_SIZE);
    let outbox_receiver_clone = outbox_receiver.clone();

    spawn_thread(
        &state,
        "*".into(),
        "send",
        |_| "sending thread".into(),
        |state| irc_send::send_main(state, outbox_receiver),
    );

    for (&server_id, server) in &state.servers {
        let outbox_sender_clone = outbox_sender.clone();

        let recv_fn = move |state: Arc<State>| -> Result<()> {
            let mut aatxe_reactor = match aatxe::IrcReactor::new() {
                Ok(r) => {
                    trace!("Successfully initialized IRC reactor.");
                    r
                }
                Err(e) => {
                    error!("Terminal error: Failed to initialize IRC reactor: {}", e);
                    return Err(e.into());
                }
            };

            let aatxe_client = {
                let server = state.servers[&server_id]
                    .read()
                    .expect(LOCK_EARLY_POISON_FAIL);

                match aatxe_reactor.prepare_client_and_connect(&server.aatxe_config) {
                    Ok(client) => {
                        trace!("Connected to server {:?}.", server.socket_addr_string);
                        client
                    }
                    Err(err) => {
                        error!(
                            "Failed to connect to server {:?}.",
                            server.socket_addr_string
                        );
                        return Err(err.into());
                    }
                }
            };

            match state
                .aatxe_clients
                .write()
                .expect(LOCK_EARLY_POISON_FAIL)
                .insert(server_id, aatxe_client.clone())
            {
                None => {}
                Some(_other_aatxe_client) => {
                    // TODO: If <https://github.com/aatxe/irc/issues/104> is resolved in favor of
                    // `IrcServer` implementing `Debug`, add the other server to this message.
                    error!(
                        "This shouldn't happen, but there was already a server registered \
                         with UUID {uuid}!",
                        uuid = server_id.uuid.hyphenated(),
                    );
                    return Err(ErrorKind::ServerRegistryClash(server_id).into());
                }
            }

            let addr = {
                let s = state.servers[&server_id]
                    .read()
                    .expect(LOCK_EARLY_POISON_FAIL);
                s.socket_addr_string.clone()
            };

            match aatxe_client.identify() {
                Ok(()) => debug!("recv[{}]: Sent identification sequence to server.", addr),
                Err(e) => error!(
                    "recv[{}]: Failed to send identification sequence to server: {}",
                    addr, e
                ),
            }

            aatxe_reactor.register_client_with_handler(aatxe_client, move |_aatxe_client, msg| {
                handle_msg(&state, server_id, &outbox_sender_clone, Ok(msg));

                Ok(())
            });

            aatxe_reactor.run().map_err(Into::into)
        };

        let addr = server
            .read()
            .expect(LOCK_EARLY_POISON_FAIL)
            .socket_addr_string
            .clone();

        spawn_thread(
            &state,
            addr,
            "recv",
            |addr| format!("receiving thread for server {addr:?}", addr = addr),
            recv_fn,
        );
    }

    while !outbox_receiver_clone.is_disconnected() {
        // TODO: Use a Condvar to enable cleaner quitting.
        thread::park_timeout(Duration::from_secs(60));
    }
}

fn handle_msg(
    state: &Arc<State>,
    server_id: ServerId,
    outbox: &irc_send::OutboxPort,
    input: Result<Message>,
) {
    match input.and_then(|msg| irc_comm::handle_msg(&state, server_id, outbox, msg)) {
        Ok(()) => {}
        Err(e) => push_to_outbox(outbox, server_id, state.handle_err_generic(e)),
    }
}

fn spawn_thread<F, PurposeF>(
    state: &Arc<State>,
    addr: String,
    purpose_desc_abbr: &str,
    purpose_desc_full: PurposeF,
    business: F,
) where
    F: FnOnce(Arc<State>) -> Result<()> + Send + 'static,
    PurposeF: FnOnce(&str) -> String,
{
    let label = format!("{}[{}]", purpose_desc_abbr, addr);

    let state_alias = state.clone();

    let thread_build_result = thread::Builder::new().name(label).spawn(move || {
        let current_thread = thread::current();
        let thread_label = current_thread.name().expect(THREAD_NAME_FAIL);

        trace!("{}: Starting....", thread_label);

        match business(state_alias) {
            Ok(()) => debug!("{}: Thread exited successfully.", thread_label),

            // TODO: Call `state.error_handler`.
            Err(err) => error!("{}: Thread exited with error: {:?}", thread_label, err),
        }
    });

    match thread_build_result {
        Ok(thread::JoinHandle { .. }) => {
            trace!("Spawned {purpose}.", purpose = purpose_desc_full(&addr));
        }
        Err(err) => match state.error_handler.run(err.into()) {
            ErrorReaction::Proceed => error!(
                "Failed to create {purpose}; ignoring.",
                purpose = purpose_desc_full(&addr),
            ),
            ErrorReaction::Quit(msg) => error!(
                "Terminal error: Failed to create {purpose}: {msg:?}",
                purpose = purpose_desc_full(&addr),
                msg = msg
            ),
        },
    }
}
