use core::*;
use irc::client::prelude::*;
use std::rc::Rc;

pub fn mk() -> Module {
    Module::new("default".into(),
                vec![ModuleFeature::Command {
                         name: "join".into(),
                         usage: "<channel>".into(),
                         handler: Rc::new(join_cmd),
                     }])
}

fn join_cmd(state: &State, &MsgMetadata { prefix, .. }: &MsgMetadata, arg: &str) -> BotCmdResult {
    match state.have_owner(prefix) {
        Ok(true) => BotCmdResult::Ok(Reaction::IrcCmd(Command::JOIN(arg.into(), None, None))),
        Ok(false) => BotCmdResult::Unauthorized,
        Err(e) => BotCmdResult::Err(e),
    }
}
