use super::BotCmdHandler;
use super::Error;
use super::Module;
use super::MsgMetadata;
use super::Reaction;
use super::Result;
use super::State;
use std;
use std::borrow::Cow;
use std::sync::Arc;
use util;
use yaml_rust::Yaml;

pub struct BotCommand {
    pub name: Cow<'static, str>,
    pub provider: Arc<Module>,
    pub auth_lvl: BotCmdAuthLvl,
    pub(super) handler: Arc<BotCmdHandler>,
    pub usage_str: Cow<'static, str>,
    pub(super) usage_yaml: Yaml,
    pub help_msg: Cow<'static, str>,
}

#[derive(Debug)]
pub enum BotCmdAttr {}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BotCmdAuthLvl {
    Public,
    Admin,
}

pub(super) fn run(
    state: &State,
    cmd_name: &str,
    cmd_args: &str,
    metadata: &MsgMetadata,
) -> Result<Option<BotCmdResult>> {
    let &BotCommand {
        ref name,
        ref provider,
        ref auth_lvl,
        ref handler,
        ref usage_yaml,
        usage_str: _,
        help_msg: _,
    } = match state.commands.get(cmd_name) {
        Some(c) => c,
        None => return Ok(None),
    };

    let user_authorized = match auth_lvl {
        &BotCmdAuthLvl::Public => Ok(true),
        &BotCmdAuthLvl::Admin => state.have_admin(metadata.prefix),
    };

    let arg = match parse_arg(usage_yaml, cmd_args) {
        Ok(arg) => arg,
        Err(res) => return Ok(Some(res)),
    };

    let result = match user_authorized {
        Ok(true) => {
            debug!("Running bot command {:?} with arg: {:?}", name, arg);
            match util::run_handler(
                "command",
                name.clone(),
                || handler.run(state, &metadata, &arg),
            ) {
                Ok(r) => r,
                Err(e) => BotCmdResult::LibErr(e),
            }
        }
        Ok(false) => BotCmdResult::Unauthorized,
        Err(e) => BotCmdResult::LibErr(e),
    };

    // TODO: Filter `QUIT`s in `irc_send` instead, and check `Reaction::RawMsg`s as well.
    match result {
        BotCmdResult::Ok(Reaction::Quit(ref s)) if *auth_lvl != BotCmdAuthLvl::Admin => {
            Ok(Some(BotCmdResult::BotErrMsg(
                format!(
                    "Only commands at authorization level {auth_lvl_owner:?} \
                     may tell the bot to quit, but the command {cmd_name:?} \
                     from module {provider_name:?}, at authorization level \
                     {cmd_auth_lvl:?}, has told the bot to quit with quit \
                     message {quit_msg:?}.",
                    auth_lvl_owner = BotCmdAuthLvl::Admin,
                    cmd_name = name,
                    provider_name = provider.name,
                    cmd_auth_lvl = auth_lvl,
                    quit_msg = s
                ).into(),
            )))
        }
        r => Ok(Some(r)),
    }
}

fn parse_arg<'s>(syntax: &'s Yaml, arg_str: &str) -> std::result::Result<Yaml, BotCmdResult> {
    use util::yaml as uy;

    match uy::parse_and_check_node(
        arg_str,
        syntax,
        "<argument>",
        || Yaml::Hash(Default::default()),
    ) {
        Ok(arg) => Ok(arg),
        Err(uy::Error(uy::ErrorKind::YamlScan(_), _)) => Err(BotCmdResult::SyntaxErr),
        Err(err) => Err(BotCmdResult::LibErr(err.into())),
    }
}
