use core::*;
use core::BotCmdAuthLvl as Auth;
use irc::client::prelude::*;

pub fn mk<'a>() -> Module<'a> {
    mk_module("default")
        .with_command("join", "<channel>", Auth::Owner, Box::new(join_cmd))
        .end()
}

fn join_cmd(_: &State, _: &MsgMetadata, arg: &str) -> BotCmdResult {
    BotCmdResult::Ok(Reaction::IrcCmd(Command::JOIN(arg.into(), None, None)))
}
