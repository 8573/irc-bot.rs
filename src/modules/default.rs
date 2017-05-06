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
        .end()
}

fn join(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    BotCmdResult::Ok(Reaction::IrcCmd(Command::JOIN(arg.to_owned(), None, None)))
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

    BotCmdResult::Ok(Reaction::IrcCmd(Command::PART(chan, comment)))
}

fn quit(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    yamlette!(read; arg.as_bytes(); [[
        {"msg" => (comment: String)}
    ]]);

    BotCmdResult::Ok(Reaction::Quit(comment.map(Into::into)))
}

fn ping(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    if arg.is_empty() {
        BotCmdResult::Ok(Reaction::Reply("pong".into()))
    } else {
        BotCmdResult::SyntaxErr
    }
}
