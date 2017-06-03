use core::*;
use core::BotCmdAuthLvl as Auth;

pub fn mk<'a>() -> Module<'a> {
    mk_module("default")
        .command("join",
                 "<channel>",
                 "Have the bot join the given channel.",
                 Auth::Admin,
                 Box::new(join))
        .command("part",
                 "{chan: '[channel]', msg: '[message]'}",
                 "Have the bot part from the given channel (defaults to the current channel), \
                  with an optional part message.",
                 Auth::Admin,
                 Box::new(part))
        .command("quit",
                 "{msg: '[message]'}",
                 "Have the bot quit.",
                 Auth::Admin,
                 Box::new(quit))
        .command("ping",
                 "",
                 "Request a short message from the bot, typically for testing purposes.",
                 Auth::Public,
                 Box::new(ping))
        .command("source",
                 "",
                 "Request information about the bot, such as the URL of a Web page about its \
                  software.",
                 Auth::Public,
                 Box::new(source))
        .command("help",
                 "{cmd: [command], list: [list name]}",
                 "Request help with the bot's features, such as commands.",
                 Auth::Public,
                 Box::new(help))
        .end()
}

fn join(_: &State, _: &MsgMetadata, arg: &str) -> Reaction {
    Reaction::RawMsg(format!("JOIN {}", arg).into())
}

fn part(state: &State,
        &MsgMetadata { target: MsgTarget(target), .. }: &MsgMetadata,
        arg: &str)
        -> BotCmdResult {
    yamlette!(read; arg.as_bytes(); [[
        {"chan" => (chan: String), "msg" => (comment: String)}
    ]]);

    let chan = match (chan, target) {
        (Some(c), _) => c,
        (None, _) if !arg.is_empty() => return BotCmdResult::SyntaxErr,
        (None, t) if t == state.nick().unwrap_or("".into()) => {
            return BotCmdResult::ArgMissing1To1("channel".into())
        }
        (None, t) => t.to_owned(),
    };

    Reaction::RawMsg(format!("PART {}{}{}",
                             chan,
                             if comment.is_some() { " :" } else { "" },
                             comment.unwrap_or_default())
                             .into())
            .into()
}

fn quit(_: &State, _: &MsgMetadata, arg: &str) -> Reaction {
    yamlette!(read; arg.as_bytes(); [[
        {"msg" => (comment: String)}
    ]]);

    Reaction::Quit(comment.map(Into::into))
}

fn ping(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    if arg.is_empty() {
        Reaction::Reply("pong".into()).into()
    } else {
        BotCmdResult::SyntaxErr
    }
}

fn source(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    let src_url = match env!("CARGO_PKG_HOMEPAGE") {
        s if !s.is_empty() => s,
        _ => "unknown",
    };

    if arg.is_empty() {
        Reaction::Reply(format!("<{}>", src_url).into()).into()
    } else {
        BotCmdResult::SyntaxErr
    }
}

fn help(state: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    yamlette!(read; arg.as_bytes(); [[
        {"cmd" => (cmd: &str), "list" => (list: &str)}
    ]]);

    let argc = [cmd, list].iter().filter(|x| x.is_some()).count();

    if argc == 0 && !arg.is_empty() {
        return BotCmdResult::SyntaxErr;
    } else if argc > 1 {
        return Reaction::Msg("Please ask for help with one thing at a time.".into()).into();
    }

    if let Some(cmd_name) = cmd {
        let &BotCommand {
                 ref name,
                 ref provider,
                 ref auth_lvl,
                 ref usage,
                 ref help_msg,
                 ..
             } = match state.command(cmd_name) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Reaction::Msg(format!("Command {:?} not found.", cmd_name).into()).into()
            }
            Err(e) => return BotCmdResult::LibErr(e),
        };

        Reaction::Msgs(vec![format!("= Help for command {:?}:", name).into(),
                            format!("- [module {:?}, auth level {:?}]", provider.name, auth_lvl)
                                .into(),
                            format!("- Syntax: {} {}", name, usage).into(),
                            help_msg.clone()]
                               .into())
                .into()
    } else if let Some(list_name) = list {
        let list_names = ["commands", "lists"];

        if list_name == "commands" {
            Reaction::Msg(format!("Available commands: {:?}", state.command_names()).into()).into()
        } else if list_name == "lists" {
            Reaction::Msg(format!("Available lists: {:?}", list_names).into()).into()
        } else {
            if list_names.contains(&list_name) {
                error!("Help list {:?} declared but not defined.", list_name);
            }

            Reaction::Msg(format!("List {:?} not found. Available lists: {:?}",
                                  list_name,
                                  list_names)
                                  .into())
                    .into()
        }
    } else {
        Reaction::Msgs(vec!["For help with a command named 'foo', try `help cmd: foo`.".into(),
                            "To see a list of all available commands, try `help list: commands`."
                                .into(),
                            format!("For this bot software's documentation, including an \
                                     introduction to the command syntax, see <{homepage}>",
                                    homepage = env!("CARGO_PKG_HOMEPAGE"))
                                    .into()]
                               .into())
                .into()
    }
}
