use core::*;
use core::BotCmdAuthLvl as Auth;
use irc::client::prelude::*;

pub fn mk<'a>() -> Module<'a> {
    mk_module("default")
        .with_command("join", "<channel>", Auth::Owner, Box::new(join))
        .with_command("part",
                      "{chan: '[channel]', msg: '[message]'}",
                      Auth::Owner,
                      Box::new(part))
        .with_command("quit", "{msg: '[message]'}", Auth::Owner, Box::new(quit))
        .with_command("ping", "", Auth::Public, Box::new(ping))
        .with_command("source", "", Auth::Public, Box::new(source))
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
