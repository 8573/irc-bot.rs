use core::BotCmdAuthLvl as Auth;
use core::*;
use regex::Captures;
use std::borrow::Cow;
use try_map::FallibleMapExt;
use util;
use util::to_cow_owned;
use util::yaml::str::YAML_STR_CHAN;
use util::yaml::str::YAML_STR_CMD;
use util::yaml::str::YAML_STR_LIST;
use util::yaml::str::YAML_STR_MSG;
use util::yaml::FW_SYNTAX_CHECK_FAIL;
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
        .trigger(
            "yes?",
            "^$",
            "Say \"Yes?\" in response to otherwise empty messages addressed to the bot.",
            TriggerPriority::Minimum,
            Box::new(empty_msg_trigger),
            &[],
        )
        .end()
}

fn join(_: HandlerContext, arg: &Yaml) -> Result<Reaction> {
    Ok(Reaction::RawMsg(
        format!(
            "JOIN {}",
            util::yaml::scalar_to_str(arg, Cow::Borrowed, "the argument to the command `join`")?
        )
        .into(),
    ))
}

fn part(
    HandlerContext {
        state,
        request_origin: MsgDest { server_id, target },
        ..
    }: HandlerContext,
    arg: &Yaml,
) -> Result<BotCmdResult> {
    let arg = arg.as_hash().expect(FW_SYNTAX_CHECK_FAIL);

    let chan = arg.get(&YAML_STR_CHAN).try_map(|y| {
        util::yaml::scalar_to_str(y, Cow::Borrowed, "the value of the parameter `chan`")
    })?;

    let chan = match (chan, target) {
        (Some(c), _) => c,
        (None, t) if t == state.nick(server_id).unwrap_or("".into()) => {
            return Ok(BotCmdResult::ArgMissing1To1("channel".into()))
        }
        (None, t) => t.into(),
    };

    let comment = arg.get(&YAML_STR_MSG).try_map(|y| {
        util::yaml::scalar_to_str(y, Cow::Borrowed, "the value of the parameter `msg`")
    })?;

    Ok(Reaction::RawMsg(
        format!(
            "PART {}{}{}",
            chan,
            if comment.is_some() { " :" } else { "" },
            comment.unwrap_or_default()
        )
        .into(),
    )
    .into())
}

fn quit(_: HandlerContext, arg: &Yaml) -> Result<Reaction> {
    let comment = arg
        .as_hash()
        .expect(FW_SYNTAX_CHECK_FAIL)
        .get(&YAML_STR_MSG)
        .try_map(|y| {
            util::yaml::scalar_to_str(y, to_cow_owned, "the value of the parameter `msg`")
        })?;

    Ok(Reaction::Quit(comment))
}

fn ping(_: HandlerContext, _: &Yaml) -> BotCmdResult {
    Reaction::Reply("pong".into()).into()
}

fn bot_fw_info(HandlerContext { state, .. }: HandlerContext, _: &Yaml) -> BotCmdResult {
    Reaction::Reply(
        format!(
            "This bot was built with `{name}.rs`, version {ver}; see <{url}>.",
            name = state.framework_crate_name(),
            ver = state.framework_version_str(),
            url = state.framework_homepage_url_str(),
        )
        .into(),
    )
    .into()
}

fn help(HandlerContext { state, .. }: HandlerContext, arg: &Yaml) -> BotCmdResult {
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
            ]
            .into(),
        )
        .into()
    } else if let Some(&Yaml::String(ref list_name)) = list {
        let list_names = ["commands", "lists"];

        if list_name == "commands" {
            Reaction::Msg(format!("Available commands: {:?}", state.command_names()).into()).into()
        } else if list_name == "lists" {
            Reaction::Msg(format!("Available lists: {:?}", list_names).into()).into()
        } else {
            if list_names.contains(&list_name.as_ref()) {
                error!("Help list {:?} declared but not defined.", list_name);
            }

            Reaction::Msg(
                format!(
                    "List {:?} not found. Available lists: {:?}",
                    list_name, list_names
                )
                .into(),
            )
            .into()
        }
    } else {
        Reaction::Msgs(
            vec![
                "For help with a command named 'foo', try `help cmd: foo`.".into(),
                "To see a list of all available commands, try `help list: commands`.".into(),
                format!(
                    "For this bot software's documentation, including an introduction to the \
                     command syntax, see <{homepage}>",
                    homepage = state.framework_homepage_url_str(),
                )
                .into(),
            ]
            .into(),
        )
        .into()
    }
}

fn empty_msg_trigger(_: HandlerContext, _: Captures) -> Reaction {
    Reaction::Msg("Yes?".into())
}
