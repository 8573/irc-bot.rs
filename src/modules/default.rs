use core::*;
use core::BotCmdAuthLvl as Auth;
use irc::client::prelude::*;

pub fn mk<'a>() -> Module<'a> {
    mk_module("default")
        .with_command("join",
                      "<channel>",
                      "Have the bot join the given channel.",
                      Auth::Owner,
                      Box::new(join))
        .with_command("part",
                      "{chan: '[channel]', msg: '[message]'}",
                      "Have the bot part from the given channel (defaults to the current \
                       channel), with an optional part message.",
                      Auth::Owner,
                      Box::new(part))
        .with_command("quit",
                      "{msg: '[message]'}",
                      "Have the bot quit.",
                      Auth::Owner,
                      Box::new(quit))
        .with_command("ping",
                      "",
                      "Request a short message from the bot, typically for testing purposes.",
                      Auth::Public,
                      Box::new(ping))
        .with_command("source",
                      "",
                      "Request information about the bot, such as the URL of a Web page about its \
                       software.",
                      Auth::Public,
                      Box::new(source))
        .with_command("help",
                      "{cmd: [command]}",
                      "Request help with the bot's features, such as commands.",
                      Auth::Public,
                      Box::new(help))
        .end()
}

fn join(_: &State, _: &MsgMetadata, arg: &str) -> Command {
    Command::JOIN(arg.to_owned(), None, None)
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
        (None, t) if t == state.nick() => return BotCmdResult::ArgMissing1To1("channel".into()),
        (None, t) => t.to_owned(),
    };

    Reaction::IrcCmd(Command::PART(chan, comment)).into()
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
        {"cmd" => (cmd: &str)}
    ]]);

    let argc = [cmd].iter().filter(|x| x.is_some()).count();

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
            Some(c) => c,
            None => {
                return Reaction::Msg(format!("Command {:?} not found.", cmd_name).into()).into()
            }
        };

        Reaction::Msgs(vec![format!("= Help for command {:?}:", name).into(),
                            format!("- [module {:?}, auth level {:?}]", provider.name, auth_lvl)
                                .into(),
                            format!("- Syntax: {} {}", name, usage).into(),
                            help_msg.clone()]
                               .into())
                .into()
    } else {
        Reaction::Msgs(vec!["For help with a command named 'foo', try `help cmd: foo`.".into(),
                            "To see a list of all available commands, try `help list: cmds`."
                                .into(),
                            format!("For this bot software's documentation, including an \
                                     introduction to the command syntax, see <{homepage}>",
                                    homepage = env!("CARGO_PKG_HOMEPAGE"))
                                    .into()]
                               .into())
                .into()
    }
}
