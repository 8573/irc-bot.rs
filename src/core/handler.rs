use super::BotCmdResult;
use super::MsgMetadata;
use super::State;
use std::panic::RefUnwindSafe;
use std::panic::UnwindSafe;
use yaml_rust::Yaml;

pub trait BotCmdHandler: Send + Sync + UnwindSafe + RefUnwindSafe {
    fn run(&self, &State, &MsgMetadata, &Yaml) -> BotCmdResult;
}

impl<F, R> BotCmdHandler for F
where
    F: Fn(&State, &MsgMetadata, &Yaml) -> R
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe,
    R: Into<BotCmdResult>,
{
    fn run(&self, state: &State, msg_md: &MsgMetadata, arg: &Yaml) -> BotCmdResult {
        self(state, msg_md, arg).into()
    }
}
