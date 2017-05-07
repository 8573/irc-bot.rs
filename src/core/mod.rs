pub use self::bot_cmd_handler::BotCmdHandler;
use irc::client::prelude::*;
use itertools::Itertools;
use std;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::cmp;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::hash::Hash;
use std::io;
use std::marker::PhantomData;
use std::path::Path;
use std::str;
use uuid::Uuid;

mod bot_cmd_handler;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    errors {
        IdentificationFailure(io_err: io::Error)
        ModuleRegistryClash(old: ModuleInfo, new: ModuleInfo)
        ModuleFeatureRegistryClash(old: ModuleFeatureInfo, new: ModuleFeatureInfo)
        Config(key: String, problem: String)
        MsgPrefixUpdateRequestedButPrefixMissing
        ModuleRequestedQuit(quit_msg: Option<Cow<'static, str>>)
    }
}

const UPDATE_MSG_PREFIX_STR: &'static str = "!!! UPDATE MESSAGE PREFIX !!!";

pub struct State<'server, 'modl> {
    server: &'server IrcServer,
    addressee_suffix: Cow<'static, str>,
    chars_indicating_msg_is_addressed_to_nick: Vec<char>,
    modules: BTreeMap<Cow<'static, str>, &'modl Module<'modl>>,
    commands: BTreeMap<Cow<'static, str>, BotCommand<'modl>>,
    msg_prefix_string: String,
}

trait GetDebugInfo {
    type Output;

    fn dbg_info(&self) -> Self::Output;
}

pub struct Module<'modl> {
    pub name: Cow<'static, str>,
    uuid: Uuid,
    features: Vec<ModuleFeature<'modl>>,
    _lifetime: PhantomData<&'modl ()>,
}

impl<'modl> PartialEq for Module<'modl> {
    fn eq(&self, other: &Self) -> bool {
        if self.uuid == other.uuid {
            debug_assert_eq!(self.name, other.name);
            true
        } else {
            false
        }
    }
}

impl<'modl> Eq for Module<'modl> {}

impl<'modl> GetDebugInfo for Module<'modl> {
    type Output = ModuleInfo;

    fn dbg_info(&self) -> ModuleInfo {
        ModuleInfo { name: self.name.to_string() }
    }
}

pub struct ModuleBuilder<'modl> {
    name: Cow<'static, str>,
    features: Vec<ModuleFeature<'modl>>,
}

pub fn mk_module<'modl, S>(name: S) -> ModuleBuilder<'modl>
    where S: Into<Cow<'static, str>>
{
    ModuleBuilder {
        name: name.into(),
        features: Default::default(),
    }
}

impl<'modl> ModuleBuilder<'modl> {
    pub fn with_command<S1, S2, S3>(mut self,
                                    name: S1,
                                    syntax: S2,
                                    help_msg: S3,
                                    auth_lvl: BotCmdAuthLvl,
                                    handler: Box<BotCmdHandler>)
                                    -> Self
        where S1: Into<Cow<'static, str>>,
              S2: Into<Cow<'static, str>>,
              S3: Into<Cow<'static, str>>
    {
        let name = name.into();

        assert!(!name.as_ref().contains(char::is_whitespace),
                "The name of the bot command {:?} contains whitespace, which is not allowed.",
                name.as_ref());

        self.features
            .push(ModuleFeature::Command {
                      name: name,
                      usage: syntax.into(),
                      help_msg: help_msg.into(),
                      auth_lvl: auth_lvl,
                      handler: handler,
                      _lifetime: PhantomData,
                  });

        self
    }

    pub fn end(self) -> Module<'modl> {
        let ModuleBuilder { name, mut features } = self;

        features.shrink_to_fit();

        Module {
            name: name,
            uuid: Uuid::new_v4(),
            features: features,
            _lifetime: PhantomData,
        }
    }
}

/// Information about a `Module` that can be gathered without needing any lifetime annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleInfo {
    name: String,
}

enum ModuleFeature<'modl> {
    Command {
        name: Cow<'static, str>,
        usage: Cow<'static, str>,
        help_msg: Cow<'static, str>,
        auth_lvl: BotCmdAuthLvl,
        handler: Box<BotCmdHandler>,
        _lifetime: PhantomData<&'modl ()>,
    },
    Trigger,
}

impl<'modl> GetDebugInfo for ModuleFeature<'modl> {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name().to_string(),
            kind: match self {
                &ModuleFeature::Command { .. } => ModuleFeatureKind::Command,
                &ModuleFeature::Trigger => ModuleFeatureKind::Trigger,
            },
        }
    }
}

/// Information about a `ModuleFeature` that can be gathered without needing any lifetime
/// annotation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFeatureInfo {
    name: String,
    kind: ModuleFeatureKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleFeatureKind {
    Command,
    Trigger,
}

impl<'modl> ModuleFeature<'modl> {
    fn name(&self) -> &str {
        match self {
            &ModuleFeature::Command { ref name, .. } => name.as_ref(),
            &ModuleFeature::Trigger => unimplemented!(),
        }
    }

    // fn provider(&self) -> &Module {
    //     match self {
    //         &ModuleFeature::Command { provider, .. } => provider,
    //         &ModuleFeature::Trigger => unimplemented!(),
    //     }
    // }
}

#[derive(Debug)]
pub enum Reaction {
    None,
    Msg(Cow<'static, str>),
    Msgs(Cow<'static, [Cow<'static, str>]>),
    Reply(Cow<'static, str>),
    Replies(Cow<'static, [Cow<'static, str>]>),
    IrcCmd(Command),
    BotCmd(Cow<'static, str>),
    Quit(Option<Cow<'static, str>>),
}

impl From<Command> for Reaction {
    fn from(c: Command) -> Self {
        Reaction::IrcCmd(c)
    }
}

pub struct BotCommand<'modl> {
    pub name: Cow<'static, str>,
    pub provider: &'modl Module<'modl>,
    pub auth_lvl: BotCmdAuthLvl,
    handler: &'modl BotCmdHandler,
    pub usage: Cow<'static, str>,
    pub help_msg: Cow<'static, str>,
}

#[derive(Debug)]
pub enum BotCmdResult {
    /// The command was processed successfully. Pass through a `Reaction`.
    Ok(Reaction),

    /// A user invoked the command without having sufficient authorization to do so. A reply will
    /// be sent informing the user of this.
    Unauthorized,

    /// A user invoked the command with incorrect syntax. A reply will be sent informing the user
    /// of the correct syntax.
    SyntaxErr,

    /// A user invoked the command without providing a required argument, named by the given
    /// string. This is a more specific version of `SyntaxErr` and should be preferred where
    /// applicable.
    ArgMissing(Cow<'static, str>),

    /// A user invoked the command in one-to-one communication (a.k.a. "query" and "PM") without
    /// providing an argument that is required only in one-to-one communication (such as a channel
    /// name, which could normally default to the name of the channel in which the command was
    /// used), named by the given string. This is a more specific version of `ArgMissing` and
    /// should be preferred where applicable.
    ArgMissing1To1(Cow<'static, str>),

    /// Pass through an instance of the framework's `Error` type.
    LibErr(Error),

    /// A user made some miscellaneous error in invoking the command. The given string will be
    /// included in a reply informing the user of their error.
    UserErrMsg(Cow<'static, str>),

    /// A miscellaneous error that doesn't seem to be the user's fault occurred while the bot was
    /// processing the command. The given string will be included in a reply informing the user of
    /// this.
    BotErrMsg(Cow<'static, str>),
}

impl From<Reaction> for BotCmdResult {
    fn from(r: Reaction) -> Self {
        BotCmdResult::Ok(r)
    }
}

impl From<Command> for BotCmdResult {
    fn from(c: Command) -> Self {
        BotCmdResult::Ok(c.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotCmdAuthLvl {
    Public,
    Owner,
}

impl<'modl> GetDebugInfo for BotCommand<'modl> {
    type Output = ModuleFeatureInfo;

    fn dbg_info(&self) -> ModuleFeatureInfo {
        ModuleFeatureInfo {
            name: self.name.to_string(),
            kind: ModuleFeatureKind::Command,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MsgTarget<'a>(pub &'a str);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MsgPrefix<'a> {
    pub nick: Option<&'a str>,
    pub user: Option<&'a str>,
    pub host: Option<&'a str>,
}

#[derive(Debug)]
pub struct MsgMetadata<'a> {
    pub target: MsgTarget<'a>,
    pub prefix: MsgPrefix<'a>,
}

#[derive(Debug)]
pub enum ErrorReaction {
    Proceed,
    Quit(Option<Cow<'static, str>>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleLoadMode {
    /// Emit an error if any of the new module's features conflict with already present modules'
    /// features.
    Add,
    /// Overwrite any already loaded features that conflict with the new module's features, if the
    /// old features were provided by a module with the same name as the new module.
    Replace,
    /// Overwrite old modules' features unconditionally.
    Force,
}

pub fn run<'modl, P, ErrF, Modls>(irc_config_json_path: P, mut error_handler: ErrF, modules: Modls)
    where P: AsRef<Path>,
          ErrF: FnMut(Error) -> ErrorReaction,
          Modls: AsRef<[Module<'modl>]>
{
    let server = match IrcServer::new(irc_config_json_path) {
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

    pub fn load_modules<Modls>(&mut self,
                               modules: Modls,
                               mode: ModuleLoadMode)
                               -> std::result::Result<(), Vec<Error>>
        where Modls: IntoIterator<Item = &'modl Module<'modl>>
    {
        let errs = modules
            .into_iter()
            .filter_map(|module| match self.load_module(module, mode) {
                            Ok(()) => None,
                            Err(e) => Some(e),
                        })
            .flatten()
            .collect::<Vec<Error>>();

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    pub fn load_module(&mut self,
                       module: &'modl Module,
                       mode: ModuleLoadMode)
                       -> std::result::Result<(), Vec<Error>> {
        debug!("Loading module {:?}, mode {:?}, providing {:?}",
               module.name,
               mode,
               module
                   .features
                   .iter()
                   .map(GetDebugInfo::dbg_info)
                   .collect::<Vec<_>>());

        if let Some(existing_module) =
            match (mode, self.modules.get(module.name.as_ref())) {
                (_, None) |
                (ModuleLoadMode::Replace, _) |
                (ModuleLoadMode::Force, _) => None,
                (ModuleLoadMode::Add, Some(old)) => Some(old),
            } {
            return Err(vec![ErrorKind::ModuleRegistryClash(existing_module.dbg_info(),
                                                           module.dbg_info())
                                    .into()]);
        }

        self.modules.insert(module.name.clone(), module);

        let errs = module
            .features
            .iter()
            .filter_map(|feature| match self.load_module_feature(module, feature, mode) {
                            Ok(()) => None,
                            Err(e) => Some(e),
                        })
            .collect::<Vec<Error>>();

        if errs.is_empty() { Ok(()) } else { Err(errs) }
    }

    fn load_module_feature(&mut self,
                           provider: &'modl Module,
                           feature: &'modl ModuleFeature,
                           mode: ModuleLoadMode)
                           -> Result<()> {
        debug!("Loading module feature (f1): {:?}", feature.dbg_info());

        if let Some(existing_feature) =
            match feature {
                &ModuleFeature::Command { .. } => {
                    match (mode, self.commands.get(feature.name())) {
                        (_, None) |
                        (ModuleLoadMode::Force, _) => None,
                        (ModuleLoadMode::Replace, Some(old)) if old.provider.name ==
                                                                provider.name => None,
                        (ModuleLoadMode::Replace, Some(old)) => Some(old.dbg_info()),
                        (ModuleLoadMode::Add, Some(old)) => Some(old.dbg_info()),
                    }
                }
                &ModuleFeature::Trigger => unimplemented!(),
            } {
            bail!(ErrorKind::ModuleFeatureRegistryClash(existing_feature, feature.dbg_info()))
        }

        self.force_load_module_feature(provider, feature);

        Ok(())
    }

    fn force_load_module_feature(&mut self,
                                 provider: &'modl Module,
                                 feature: &'modl ModuleFeature) {
        debug!("Loading module feature (f2): {:?}", feature.dbg_info());

        match feature {
            &ModuleFeature::Command {
                 ref name,
                 ref handler,
                 ref auth_lvl,
                 ref usage,
                 ref help_msg,
                 _lifetime: _,
             } => {
                self.commands
                    .insert(name.clone(),
                            BotCommand {
                                provider: provider,
                                name: name.clone(),
                                auth_lvl: auth_lvl.clone(),
                                handler: handler.as_ref(),
                                usage: usage.clone(),
                                help_msg: help_msg.clone(),
                            })
            }
            &ModuleFeature::Trigger => unimplemented!(),
        };
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
            match handle_msg(self, msg).map_err(|err| match err {
                                                    Error(ErrorKind::ModuleRequestedQuit(msg),
                                                          _) => ErrorReaction::Quit(msg),
                                                    e => error_handler(e),
                                                }) {
                Ok(()) => {}
                Err(ErrorReaction::Proceed) => {}
                Err(ErrorReaction::Quit(msg)) => {
                    self.quit(msg);
                    break 'main_loop;
                }
            }
        }
    }

    pub fn say<S1, S2>(&self, MsgTarget(target): MsgTarget, addressee: S1, msg: S2) -> Result<()>
        where S1: Borrow<str>,
              S2: Display
    {
        let final_msg = format!(
            "{}{}{}",
            addressee.borrow(),
            if addressee.borrow().is_empty() {
                ""
            } else {
                &self.addressee_suffix
            },
            msg,
        );
        info!("Sending message to {:?}: {:?}", target, final_msg);
        self.wrap_msg(target, &final_msg, |line| {
            self.server
                .send_privmsg(target, line)
                .map_err(Into::into)
        })
    }

    fn wrap_msg<F>(&self, target: &str, msg: &str, mut f: F) -> Result<()>
        where F: FnMut(&str) -> Result<()>
    {
        // :nick!user@host PRIVMSG target :message
        // :nick!user@host NOTICE target :message
        let raw_len_limit = 512;
        let punctuation_len = {
            let line_terminator_len = 2;
            let spaces = 3;
            let colons = 2;
            colons + spaces + line_terminator_len
        };
        let prefix_len = self.msg_prefix_string.len();
        let cmd_len = "PRIVMSG".len();
        let metadata_len = prefix_len + cmd_len + target.len() + punctuation_len;
        let msg_len_limit = raw_len_limit - metadata_len;

        if msg.len() < msg_len_limit {
            return f(msg);
        }

        let mut split_end_idx = 0;

        let lines = msg.match_indices(char::is_whitespace)
            .peekable()
            .batching(|mut iter| {
                debug_assert!(msg.len() >= msg_len_limit);

                let split_start_idx = split_end_idx;

                if split_start_idx >= msg.len() {
                    return None;
                }

                while let Some(&(next_space_idx, _)) = iter.peek() {
                    if msg[split_start_idx..next_space_idx].len() < msg_len_limit {
                        split_end_idx = next_space_idx;
                        iter.next();
                    } else {
                        break;
                    }
                }

                if iter.peek().is_none() {
                    split_end_idx = msg.len()
                } else if split_end_idx <= split_start_idx {
                    split_end_idx = cmp::min(split_start_idx + msg_len_limit, msg.len())
                }

                Some(msg[split_start_idx..split_end_idx].trim())
            });

        for line in lines {
            f(line)?
        }

        Ok(())
    }

    fn quit<'a>(&self, msg: Option<Cow<'a, str>>) {
        let default_quit_msg = format!("<{}> v{}",
                                       env!("CARGO_PKG_HOMEPAGE"),
                                       env!("CARGO_PKG_VERSION"));

        info!("Quitting. Quit message: {:?}.", msg);

        self.server
            .send_quit(msg.unwrap_or(default_quit_msg.into()).as_ref())
            .unwrap_or_else(|err| error!("Error while quitting: {:?}", err))
    }

    pub fn nick(&self) -> &str {
        self.server.current_nickname()
    }

    pub fn command(&self, name: &str) -> Option<&BotCommand> {
        self.commands.get(name)
    }

    pub fn command_names(&self) -> Vec<Cow<'static, str>> {
        self.commands.keys().cloned().collect()
    }

    pub fn have_module_feature(&self, kind: ModuleFeatureKind, name: &str) -> bool {
        match kind {
            ModuleFeatureKind::Command => self.commands.contains_key(name),
            ModuleFeatureKind::Trigger => unimplemented!(),
        }
    }

    pub fn have_owner(&self, MsgPrefix { nick, user, .. }: MsgPrefix) -> Result<bool> {
        let cfg_key = "owner-auth-check-policy".to_string();
        let default = "nick-only".to_string();
        let policy = self.query_cfg(&cfg_key).unwrap_or(&default);
        let (match_nick, match_user) = match () {
            () if policy == "nick+user" => (true, true),
            () if policy == "nick-only" => (true, false),
            () if policy == "user-only" => (false, true),
            _ => {
                bail!(ErrorKind::Config(cfg_key,
                                        "is not `nick+user`, `nick-only`, or `user-only`".into()))
            }
        };

        Ok(match self.server.config().owners {
               Some(ref vec) => {
                   vec.iter()
                       .map(String::as_ref)
                       .map(Some)
                       .any(|owner| {
                                (!match_nick || owner == nick) && (!match_user || owner == user)
                            })
               }
               None => false,
           })
    }

    pub fn query_cfg<Q>(&self, key: &Q) -> Option<&String>
        where String: Borrow<Q>,
              Q: Eq + Hash
    {
        if let Some(ref options) = self.server.config().options {
            options.get(key)
        } else {
            None
        }
    }

    fn handle_reaction(&self, msg_md: &MsgMetadata, reaction: Reaction) -> Result<()> {
        let &MsgMetadata {
                 target,
                 prefix: MsgPrefix { nick, .. },
             } = msg_md;

        let (reply_target, reply_addressee) = if target.0 == self.nick() {
            (MsgTarget(nick.unwrap()), "")
        } else {
            (target, nick.unwrap_or(""))
        };

        match reaction {
            Reaction::None => Ok(()),
            Reaction::Msg(s) => self.say(reply_target, "", &s),
            Reaction::Msgs(a) => {
                for s in a.iter() {
                    self.say(reply_target, "", &s)?
                }
                Ok(())
            }
            Reaction::Reply(s) => self.say(reply_target, reply_addressee, &s),
            Reaction::Replies(a) => {
                for s in a.iter() {
                    self.say(reply_target, reply_addressee, &s)?
                }
                Ok(())
            }
            Reaction::IrcCmd(c) => {
                match self.server.send(c) {
                    Ok(()) => Ok(()),
                    Err(e) => bail!(e),
                }
            }
            Reaction::BotCmd(cmd_ln) => self.handle_bot_command(msg_md, cmd_ln),
            Reaction::Quit(msg) => bail!(ErrorKind::ModuleRequestedQuit(msg)),
        }
    }

    fn handle_bot_command<C>(&self, msg_md: &MsgMetadata, command_line: C) -> Result<()>
        where C: Borrow<str>
    {
        let cmd_ln = command_line.borrow();

        debug_assert!(!cmd_ln.trim().is_empty());

        let mut cmd_name_and_args = cmd_ln.splitn(2, char::is_whitespace);
        let cmd_name = cmd_name_and_args.next().unwrap_or("");
        let cmd_args = cmd_name_and_args.next().unwrap_or("");

        self.handle_reaction(msg_md,
                             self.bot_command_reaction(msg_md, cmd_name, cmd_args))
    }

    fn run_bot_command(&self, msg_md: &MsgMetadata, &BotCommand {
                 ref name,
                 ref provider,
                 ref auth_lvl,
                 ref handler,
                 usage: _,
                 help_msg: _,
}: &BotCommand, cmd_args: &str) -> BotCmdResult{

        let user_authorized = match auth_lvl {
            &BotCmdAuthLvl::Public => Ok(true),
            &BotCmdAuthLvl::Owner => self.have_owner(msg_md.prefix),
        };

        let result = match user_authorized {
            Ok(true) => handler.run(self, msg_md, cmd_args),
            Ok(false) => BotCmdResult::Unauthorized,
            Err(e) => BotCmdResult::LibErr(e),
        };

        match result {
            BotCmdResult::Ok(Reaction::Quit(ref s)) if *auth_lvl != BotCmdAuthLvl::Owner => {
                BotCmdResult::BotErrMsg(format!("Only commands at authorization level \
                                                 {auth_lvl_owner:?} may tell the bot to quit, \
                                                 but the command {cmd_name:?} from module \
                                                 {provider_name:?}, at authorization level \
                                                 {cmd_auth_lvl:?}, has told the bot to quit with \
                                                 quit message {quit_msg:?}.",
                                                auth_lvl_owner = BotCmdAuthLvl::Owner,
                                                cmd_name = name,
                                                provider_name = provider.name,
                                                cmd_auth_lvl = auth_lvl,
                                                quit_msg = s)
                                                .into())
            }
            r => r,
        }
    }

    fn bot_command_reaction(&self,
                            msg_md: &MsgMetadata,
                            cmd_name: &str,
                            cmd_args: &str)
                            -> Reaction {
        let cmd = match self.commands.get(cmd_name) {
            Some(c) => c,
            None => {
                return Reaction::Reply(format!("Unknown command {:?}; apologies.", cmd_name).into())
            }
        };

        let &BotCommand {
                 ref name,
                 ref usage,
                 ..
             } = cmd;

        let cmd_result = match self.run_bot_command(msg_md, cmd, cmd_args) {
            BotCmdResult::Ok(r) => Ok(r),
            BotCmdResult::Unauthorized => {
                Err(format!("My apologies, but you do not appear to have sufficient authority to \
                             use my {:?} command.",
                            name))
            }
            BotCmdResult::SyntaxErr => Err(format!("Syntax: {} {}", name, usage)),
            BotCmdResult::ArgMissing(arg_name) => {
                Err(format!("Syntax error: For command {:?}, the argument {:?} is required, but \
                             it was not given.",
                            name,
                            arg_name))
            }
            BotCmdResult::ArgMissing1To1(arg_name) => {
                Err(format!("Syntax error: When command {:?} is used outside of a channel, the \
                             argument {:?} is required, but it was not given.",
                            name,
                            arg_name))
            }
            BotCmdResult::LibErr(e) => Err(format!("Error: {}", e)),
            BotCmdResult::UserErrMsg(s) => Err(format!("User error: {}", s)),
            BotCmdResult::BotErrMsg(s) => Err(format!("Internal error: {}", s)),
        };

        match cmd_result {
            Ok(r) => r,
            Err(s) => Reaction::Reply(s.into()),
        }
    }
}

fn handle_msg(state: &mut State, input_msg: io::Result<Message>) -> Result<()> {
    let raw_msg = match input_msg {
        Ok(m) => m,
        Err(e) => bail!(e),
    };

    debug!("{:?}", raw_msg);

    (match raw_msg.command {
         Command::PRIVMSG(..) => handle_privmsg,
         Command::NOTICE(..) => ignore_msg,
         Command::Response(Response::RPL_ENDOFMOTD, _, _) => handle_end_of_motd,
         _ => ignore_msg,
     })(state, raw_msg)
}

fn handle_privmsg(state: &mut State, raw_msg: Message) -> Result<()> {
    let Message {
        tags: _,
        ref prefix,
        ref command,
    } = raw_msg;

    let (target, msg) = match parse_msg_to_nick(state, command, state.nick()) {
        Some((t, m)) => (t, m),
        None => return Ok(()),
    };

    info!("{:?}", raw_msg);

    let msg_md = MsgMetadata {
        target: target,
        prefix: parse_prefix(prefix),
    };

    if msg.is_empty() {
        state.handle_reaction(&msg_md, Reaction::Reply("Yes?".into()))
    } else if msg_md.prefix.nick == Some(target.0) && msg == UPDATE_MSG_PREFIX_STR {
        if let Some(s) = prefix.to_owned() {
            info!("Setting stored message prefix to {:?}", s);
            state.msg_prefix_string = s;
            Ok(())
        } else {
            Err(ErrorKind::MsgPrefixUpdateRequestedButPrefixMissing.into())
        }
    } else {
        state.handle_bot_command(&msg_md, msg)
    }
}

fn handle_end_of_motd(state: &mut State, _: Message) -> Result<()> {
    state.say(MsgTarget(state.nick()), state.nick(), UPDATE_MSG_PREFIX_STR)
}

fn ignore_msg(_: &mut State, _: Message) -> Result<()> {
    Ok(())
}

fn is_msg_to_nick(state: &State, MsgTarget(target): MsgTarget, msg: &str, nick: &str) -> bool {
    target == nick || msg == nick ||
    (msg.starts_with(nick) &&
     (msg.find(|c: char| {
                   state
                       .chars_indicating_msg_is_addressed_to_nick
                       .contains(&c)
               }) == Some(nick.len())))
}

fn user_msg(cmd: &Command) -> Option<(MsgTarget, &String)> {
    match cmd {
        &Command::PRIVMSG(ref target, ref msg) |
        &Command::NOTICE(ref target, ref msg) => Some((MsgTarget(target), msg)),
        _ => None,
    }
}

fn parse_msg_to_nick<'c>(state: &State,
                         cmd: &'c Command,
                         nick: &str)
                         -> Option<(MsgTarget<'c>, &'c str)> {
    user_msg(cmd).and_then(|(target, msg)| if is_msg_to_nick(state, target, msg, nick) {
                               Some((target,
                                     msg.trim_left_matches(nick)
                                         .trim_left_matches(|c: char| {
                                                                state
                            .chars_indicating_msg_is_addressed_to_nick
                            .contains(&c)
                                                            })
                                         .trim()))
                           } else {
                               None
                           })
}

fn parse_prefix(prefix: &Option<String>) -> MsgPrefix {
    let prefix = match prefix {
        &Some(ref s) => s,
        &None => return MsgPrefix::default(),
    };
    let mut iter = prefix.rsplitn(2, '@');
    let host = iter.next();
    let mut iter = iter.next().unwrap_or("").splitn(2, '!');
    let nick = iter.next();
    let user = iter.next();
    MsgPrefix {
        nick: nick,
        user: user,
        host: host,
    }
}
