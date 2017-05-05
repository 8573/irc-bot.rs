use core::*;
use irc::client::prelude::*;

pub fn mk<'a>() -> Module<'a> {
    mk_module("default")
        .with_command("join", "<channel>", Box::new(join_cmd))
        .end()
}

fn join_cmd(state: &State, &MsgMetadata { prefix, .. }: &MsgMetadata, arg: &str) -> BotCmdResult {
    match state.have_owner(prefix) {
        Ok(true) => BotCmdResult::Ok(Reaction::IrcCmd(Command::JOIN(arg.into(), None, None))),
        Ok(false) => BotCmdResult::Unauthorized,
        Err(e) => BotCmdResult::Err(e),
    }
}
