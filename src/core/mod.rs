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
use crossbeam;
use futures::Future;
use futures::Sink;
use futures::Stream;
use futures::stream;
use parking_lot::Mutex;
use parking_lot::RwLock;
use pircolate;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::mpsc;
use tokio_core;
use tokio_irc_client;

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

const MSG_OUTBOX_SIZE: usize = 256;

pub struct State<'server, 'modl> {
    _lifetime_server: PhantomData<&'server ()>,
    config: Config,
    mpsc_channel: Mutex<mpsc::SyncSender<pircolate::Message>>,
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
    username: Option<String>,
    realname: Option<String>,
    admins: Vec<config::Admin>,
    servers: Vec<config::Server>,
    channels: Vec<String>,
}

impl<'server, 'modl> State<'server, 'modl> {
    fn new<ErrF>(config: Config, error_handler: ErrF) -> State<'server, 'modl>
        where ErrF: 'static + Fn(Error) -> ErrorReaction + Send + Sync
    {
        let nick = config.nick.clone();
        let username = config.username.clone();

        State {
            _lifetime_server: PhantomData,
            config: config,
            // All messages will be ignored (although none are expected to be sent) until this
            // channel is replaced with a useful one in `State::run`.
            mpsc_channel: Mutex::new(mpsc::sync_channel(0).0),
            addressee_suffix: ": ".into(),
            chars_indicating_msg_is_addressed_to_nick: vec![':', ','],
            modules: Default::default(),
            commands: Default::default(),
            msg_prefix: RwLock::new(OwningMsgPrefix::from_string(format!("{}!{}@",
                                                                 nick,
                                                                 username.unwrap_or_default()))),
            error_handler: Arc::new(error_handler),
        }
    }

    fn run(mut self, mut reactor_core: tokio_core::reactor::Core) {
        info!("Loaded modules: {:?}",
              self.modules.keys().collect::<Vec<_>>());
        info!("Loaded commands: {:?}",
              self.commands.keys().collect::<Vec<_>>());
        trace!("Running bot....");

        let error_handler = self.error_handler.clone();

        let connection_sequence = match irc_comm::connection_sequence(&self) {
            Ok(v) => v,
            Err(e) => {
                error_handler(e.into());
                error!("Terminal error: Failed to construct connection sequence messages.");
                return;
            }
        };

        // There shouldn't be multiple extant references to this, but Rust can't tell that the
        // closures that capture it run serially.
        let server_sinks = RwLock::new(Vec::new());

        let (send_chan, recv_chan) = mpsc::sync_channel(MSG_OUTBOX_SIZE);

        self.mpsc_channel = Mutex::new(send_chan);

        let ref server = self.config.servers[0];

        let conn_init_future = tokio_irc_client::Client::new(server.resolve())
            .connect_tls(&reactor_core.handle(), server.host.clone())
            .from_err::<Error>()
            .map(|irc_transport| {
                     irc_transport
                         .from_err::<Error>()
                         .sink_from_err::<Error>()
                 })
            .and_then(|irc_transport| {
                debug!("[{}] Sending connection sequence: {:#?}",
                       server.socket_addr_string(),
                       connection_sequence
                           .iter()
                           .map(|m| m.raw_message())
                           .collect::<Vec<_>>());

                stream::iter(connection_sequence.into_iter().map(Ok)).forward(irc_transport)
            });

        let (sink, stream) = match reactor_core.run(conn_init_future) {
            Ok((stream::Iter { .. }, irc_transport)) => {
                trace!("[{}] Connection sequence sent.",
                       server.socket_addr_string());
                irc_transport.split()
            }
            Err(e) => {
                error_handler(e);
                error!("Terminal error: Failed to send connection sequence.");
                return;
            }
        };

        server_sinks.write().push(sink);

        let intake_future = stream.for_each(|msg| Ok(handle_msg(&self, msg)));

        let outgoing_msgs_iter = recv_chan
            .into_iter()
            .filter_map(|msg| irc_send::process_outgoing_msg(&self, msg))
            .map(Ok);

        let server_sink = server_sinks.write().remove(0);

        let output_future = stream::iter(outgoing_msgs_iter).forward(server_sink);

        let error_handler_2 = error_handler.clone();

        crossbeam::scope(move |scope| {
            scope.spawn(move || {
                let mut output_core = match tokio_core::reactor::Core::new() {
                    Ok(r) => {
                        trace!("Initialized Tokio reactor core for output thread.");
                        r
                    }
                    Err(e) => {
                        error_handler_2(e.into());
                        error!("Terminal error in Tokio: Failed to initialize reactor core for \
                                output thread.");
                        return Err(());
                    }
                };

                match output_core.run(output_future) {
                    Ok((_, _)) => {
                        trace!("Output reactor core shutting down without error.");
                        Ok(())
                    }
                    Err(e) => {
                        error_handler_2(e);
                        error!("Terminal error in output reactor core.");
                        Err(())
                    }
                }
            });

            match reactor_core.run(intake_future) {
                Ok(()) => {
                    trace!("Intake reactor core shutting down without error.");
                }
                Err(e) => {
                    error_handler(e);
                    error!("Terminal error in intake reactor core.");
                }
            }
        })
    }

    fn handle_err<E, S>(&self, err: E, desc: S)
        where E: Into<Error>,
              S: Borrow<str>
    {
        let desc = desc.borrow();

        let reaction = match err.into() {
            Error(ErrorKind::ModuleRequestedQuit(msg), _) => ErrorReaction::Quit(msg),
            e => (self.error_handler)(e),
        };

        match reaction {
            ErrorReaction::Proceed => {
                trace!("Proceeding despite error{}{}{}.",
                       if desc.is_empty() { "" } else { " (" },
                       desc,
                       if desc.is_empty() { "" } else { ")" })
            }
            ErrorReaction::Quit(msg) => irc_comm::quit(self, msg),
        }
    }

    fn handle_err_generic<E>(&self, err: E)
        where E: Into<Error>
    {
        self.handle_err(err, "")
    }
}

pub fn run<'modl, Cfg, ErrF, Modls>(config: Cfg, error_handler: ErrF, modules: Modls)
    where Cfg: config::IntoConfig,
          ErrF: 'static + Fn(Error) -> ErrorReaction + Send + Sync,
          Modls: AsRef<[Module<'modl>]>
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

    let reactor = match tokio_core::reactor::Core::new() {
        Ok(r) => {
            trace!("Initialized Tokio reactor core.");
            r
        }
        Err(e) => {
            error_handler(e.into());
            error!("Terminal error in Tokio: Failed to initialize reactor core.");
            return;
        }
    };

    let mut state = State::new(config, error_handler);

    match state.load_modules(modules.as_ref().iter(), ModuleLoadMode::Add) {
        Ok(()) => {}
        Err(errs) => {
            for err in errs {
                match (state.error_handler)(err) {
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

    state.run(reactor);
}

fn handle_msg(state: &State, msg: pircolate::Message) {
    match irc_comm::handle_msg(&state, msg) {
        Ok(()) => {}
        Err(e) => state.handle_err_generic(e),
    }
}
