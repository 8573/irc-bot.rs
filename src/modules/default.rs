use core::*;
use core::BotCmdAuthLvl as Auth;
use std::borrow::Cow;
use util;
use yaml_rust::Yaml;

pub fn mk() -> Module {
    mk_module("default")
        .command(
            "join",
            "<channel>",
            "Have the bot join the given channel. Note that a channel name containing the \
             character '#' will need to be enclosed in quotation marks, like '#channel' or \
             \"#channel\".",
            Auth::Admin,
            Box::new(join),
            &[],
        )
        .command(
            "part",
            "{chan: '[channel]', msg: '[message]'}",
            "Have the bot part from the given channel (defaults to the current channel), with an \
             optional part message.",
            Auth::Admin,
            Box::new(part),
            &[],
        )
        .command(
            "quit",
            "{msg: '[message]'}",
            "Have the bot quit.",
            Auth::Admin,
            Box::new(quit),
            &[],
        )
        .command(
            "ping",
            "",
            "Request a short message from the bot, typically for testing purposes.",
            Auth::Public,
            Box::new(ping),
            &[],
        )
        .command(
            "framework-info",
            "",
            "Request information about the framework with which the bot was built, such as the URL \
             of a Web page about it.",
            Auth::Public,
            Box::new(bot_fw_info),
            &[],
        )
        .command(
            "help",
            "{cmd: '[command]', list: '[list name]'}",
            "Request help with the bot's features, such as commands.",
            Auth::Public,
            Box::new(help),
            &[],
        )
        .end()
}

static FW_SYNTAX_CHECK_FAIL: &str =
    "The framework should have caught this syntax error before it tried to run this command \
     handler!";

lazy_static! {
    static ref YAML_STR_CHAN: Yaml = Yaml::String("chan".into());
    static ref YAML_STR_CMD: Yaml = Yaml::String("cmd".into());
    static ref YAML_STR_LIST: Yaml = Yaml::String("list".into());
    static ref YAML_STR_MSG: Yaml = Yaml::String("msg".into());
}

fn join(_: &State, _: &MsgMetadata, arg: &Yaml) -> Reaction {
    Reaction::RawMsg(
        format!(
            "JOIN {}",
            util::yaml::scalar_to_str(arg, Cow::Borrowed).expect(FW_SYNTAX_CHECK_FAIL)
        ).into(),
    )
}

fn part(
    state: &State,
    &MsgMetadata { target: MsgTarget(msg_target), .. }: &MsgMetadata,
    arg: &Yaml,
) -> BotCmdResult {
    let arg = arg.as_hash().expect(FW_SYNTAX_CHECK_FAIL);

    let chan = arg.get(&YAML_STR_CHAN).map(|y| {
        util::yaml::scalar_to_str(y, Cow::Borrowed).expect(FW_SYNTAX_CHECK_FAIL)
    });

    let chan = match (chan, msg_target) {
        (Some(c), _) => c,
        (None, t) if t == state.nick().unwrap_or("".into()) => {
            return BotCmdResult::ArgMissing1To1("channel".into())
        }
        (None, t) => t.into(),
    };

    let comment = arg.get(&YAML_STR_MSG).map(|y| {
        util::yaml::scalar_to_str(y, Cow::Borrowed).expect(FW_SYNTAX_CHECK_FAIL)
    });

    Reaction::RawMsg(
        format!(
            "PART {}{}{}",
            chan,
            if comment.is_some() { " :" } else { "" },
            comment.unwrap_or_default()
        ).into(),
    ).into()
}

fn quit(_: &State, _: &MsgMetadata, arg: &Yaml) -> Reaction {
    let comment = arg.as_hash()
        .expect(FW_SYNTAX_CHECK_FAIL)
        .get(&YAML_STR_MSG)
        .map(|y| {
            util::yaml::scalar_to_str(y, |s| Cow::Owned(s.to_owned())).expect(FW_SYNTAX_CHECK_FAIL)
        });

    Reaction::Quit(comment)
}

fn ping(_: &State, _: &MsgMetadata, _: &Yaml) -> BotCmdResult {
    Reaction::Reply("pong".into()).into()
}

fn bot_fw_info(_: &State, _: &MsgMetadata, _: &Yaml) -> BotCmdResult {
    fn d(l: &'static [&'static str]) -> &'static str {
        l.iter().find(|s| !s.is_empty()).unwrap_or(&"unknown")
    }

    Reaction::Reply(
        format!(
            "This bot was built with `{name}.rs`, version {ver}; see <{url}>.",
            name = d(&[env!("CARGO_PKG_NAME")]),
            ver = d(&[env!("IRC_BOT_RS_GIT_VERSION"), env!("CARGO_PKG_VERSION")]),
            url = d(&[env!("CARGO_PKG_HOMEPAGE")])
        ).into(),
    ).into()
}

fn help(state: &State, _: &MsgMetadata, arg: &Yaml) -> BotCmdResult {
    let arg = arg.as_hash();

    let cmd = arg.and_then(|m| m.get(&YAML_STR_CMD));
    let list = arg.and_then(|m| m.get(&YAML_STR_LIST));

    if [cmd, list].iter().filter(|x| x.is_some()).count() > 1 {
        return Reaction::Msg("Please ask for help with one thing at a time.".into()).into();
    }

    if let Some(&Yaml::String(ref cmd_name)) = cmd {
        let &BotCommand {
            ref name,
            ref provider,
            ref auth_lvl,
            ref usage_str,
            ref help_msg,
            ..
        } = match state.command(cmd_name) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Reaction::Msg(format!("Command {:?} not found.", cmd_name).into()).into()
            }
            Err(e) => return BotCmdResult::LibErr(e),
        };

        Reaction::Msgs(
            vec![
                format!("= Help for command {:?}:", name).into(),
                format!("- [module {:?}, auth level {:?}]", provider.name, auth_lvl).into(),
                format!("- Syntax: {} {}", name, usage_str).into(),
                help_msg.clone(),
            ].into(),
        ).into()
    } else if let Some(&Yaml::String(ref list_name)) = list {
        let list_names = ["commands", "lists"];

        if list_name == "commands" {
            Reaction::Msg(
                format!("Available commands: {:?}", state.command_names()).into(),
            ).into()
        } else if list_name == "lists" {
            Reaction::Msg(format!("Available lists: {:?}", list_names).into()).into()
        } else {
            if list_names.contains(&list_name.as_ref()) {
                error!("Help list {:?} declared but not defined.", list_name);
            }

            Reaction::Msg(
                format!(
                    "List {:?} not found. Available lists: {:?}",
                    list_name,
                    list_names
                ).into(),
            ).into()
        }
    } else {
        Reaction::Msgs(
            vec![
                "For help with a command named 'foo', try `help cmd: foo`.".into(),
                "To see a list of all available commands, try `help list: commands`.".into(),
                format!(
                    "For this bot software's documentation, including an introduction to the \
                     command syntax, see <{homepage}>",
                    homepage = env!("CARGO_PKG_HOMEPAGE")
                ).into(),
            ].into(),
        ).into()
    }
}
