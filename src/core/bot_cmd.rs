use super::BotCmdHandler;
use super::Error;
use super::Module;
use super::MsgMetadata;
use super::Reaction;
use super::Result;
use super::State;
use irc;
use rand;
use regex;
use serde_yaml;
use std;
use std::borrow::Cow;
use std::io;
use std::num::ParseIntError;
use std::sync::Arc;
use util;
use walkdir;
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

    /// A user used the specified parameter of the command without having sufficient authorization
    /// to do so. A reply will be sent informing the user of this.
    ParamUnauthorized(Cow<'static, str>),

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

impl From<Error> for BotCmdResult {
    fn from(e: Error) -> Self {
        BotCmdResult::LibErr(e)
    }
}

macro_rules! impl_from_err_for_bot_cmd_result {
    ($err:ty) => {
        impl From<$err> for BotCmdResult {
            fn from(e: $err) -> Self {
                Error::from(e).into()
            }
        }
    };
}

// Implement `From<E>` for `BotCmdResult` for all error types `E` for which our `Error` implements
// `From<E>`.
// TODO: I should be able to quantify over those types once specialization is stable, I think.
impl_from_err_for_bot_cmd_result!(ParseIntError);
impl_from_err_for_bot_cmd_result!(io::Error);
impl_from_err_for_bot_cmd_result!(irc::error::IrcError);
impl_from_err_for_bot_cmd_result!(rand::Error);
impl_from_err_for_bot_cmd_result!(regex::Error);
impl_from_err_for_bot_cmd_result!(serde_yaml::Error);
impl_from_err_for_bot_cmd_result!(util::yaml::Error);
impl_from_err_for_bot_cmd_result!(walkdir::Error);

impl<T, E> From<std::result::Result<T, E>> for BotCmdResult
where
    T: Into<BotCmdResult>,
    E: Into<BotCmdResult>,
{
    fn from(result: std::result::Result<T, E>) -> Self {
        match result {
            Ok(x) => x.into(),
            Err(e) => e.into(),
        }
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
            match util::run_handler("command", name.clone(), || {
                handler.run(state, &metadata, &arg)
            }) {
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

    match uy::parse_and_check_node(arg_str, syntax, "<argument>", || {
        Yaml::Hash(Default::default())
    }) {
        Ok(arg) => Ok(arg),
        Err(uy::Error(uy::ErrorKind::YamlScan(_), _)) => Err(BotCmdResult::SyntaxErr),
        Err(err) => Err(BotCmdResult::LibErr(err.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use util::yaml::mk_str as s;

    fn pa(syntax_str: &str, arg_str: &str) -> std::result::Result<Yaml, String> {
        parse_arg(
            &util::yaml::parse_node(syntax_str)
                .unwrap()
                .unwrap_or(Yaml::Hash(Default::default())),
            arg_str,
        ).map_err(|err| format!("{:?}", err))
    }

    fn map<'a, I>(entries: I) -> Yaml
    where
        I: IntoIterator<Item = &'a (Yaml, Yaml)>,
    {
        util::yaml::mk_map(entries.into_iter().cloned())
    }

    fn seq<'a, I>(entries: I) -> Yaml
    where
        I: IntoIterator<Item = &'a Yaml>,
    {
        util::yaml::mk_seq(entries.into_iter().cloned())
    }

    // TODO: Turn this into a doctest.
    #[test]
    fn parse_arg_examples() {
        assert_eq!(pa("", ""), Ok(map(&[])));
        assert_eq!(pa("{}", ""), Ok(map(&[])));
        assert_eq!(pa("{k: '[v]'}", ""), Ok(map(&[])));
        assert_eq!(pa("{k: '[v]'}", "k: x"), Ok(map(&[(s("k"), s("x"))])));
        assert_eq!(
            pa("{k: '[v]'}", "k: 1"),
            Ok(map(&[(s("k"), Yaml::Integer(1))]))
        );
        assert!(pa("{k: '[v]'}", "k: {}").is_err());
        assert_eq!(pa("{k: v}", "k: x"), Ok(map(&[(s("k"), s("x"))])));
        assert!(pa("{k: v}", "").is_err());
        assert_eq!(
            pa("{k: [a]}", "k: [b]"),
            Ok(map(&[(s("k"), seq(&[s("b")]))]))
        );
        assert_eq!(pa("{k: [a]}", ""), Ok(map(&[])));
        assert!(pa("{k: [a]}", "k: x").is_err());
        assert!(pa("{k: '[v]', j: v}", "").is_err());
        assert_eq!(pa("{k: '[v]', j: v}", "j: x"), Ok(map(&[(s("j"), s("x"))])));
        assert_eq!(pa("{k: {j: '[v]'}}", ""), Ok(map(&[])));
        assert!(pa("{k: {j: v}}", "").is_err());
        assert_eq!(
            pa("{k: {j: v}}", "k: {j: x}"),
            Ok(map(&[(s("k"), map(&[(s("j"), s("x"))]))]))
        );
        assert_eq!(pa("{k: ...}", "k: x"), Ok(map(&[(s("k"), s("x"))])));
        assert_eq!(pa("{k: '[...]'}", "k: x"), Ok(map(&[(s("k"), s("x"))])));
        assert_eq!(
            pa("{k: '[...]'}", "k: 1"),
            Ok(map(&[(s("k"), Yaml::Integer(1))]))
        );
        assert_eq!(pa("{k: '[...]'}", "k: {}"), Ok(map(&[(s("k"), map(&[]))])));
        assert_eq!(pa("{k: '[...]'}", "k: []"), Ok(map(&[(s("k"), seq(&[]))])));
        assert_eq!(pa("{k: '[...]'}", ""), Ok(map(&[])));
        assert!(pa("{k: ...}", "").is_err());
        assert_eq!(
            pa("{k: ...}", "k: [b]"),
            Ok(map(&[(s("k"), seq(&[s("b")]))]))
        );
        assert_eq!(
            pa("{k: ...}", "k: {j: x}"),
            Ok(map(&[(s("k"), map(&[(s("j"), s("x"))]))]))
        );
        assert!(pa("{k: {j: ...}}", "").is_err());
        assert_eq!(
            pa("{k: {j: ...}}", "k: {j: 123}"),
            Ok(map(&[(s("k"), map(&[(s("j"), Yaml::Integer(123))]))]))
        );
    }
}
