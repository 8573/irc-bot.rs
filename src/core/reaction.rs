use irc::client::prelude::*;
use std::borrow::Cow;

#[derive(Debug)]
pub enum Reaction {
    None,
    Msg(Cow<'static, str>),
    Msgs(Cow<'static, [Cow<'static, str>]>),
    Reply(Cow<'static, str>),
    Replies(Cow<'static, [Cow<'static, str>]>),
    IrcCmd(Command),
    BotCmd(Cow<'static, str>),
    Quit(Option<Cow<'static, str>>),
}

impl From<Command> for Reaction {
    fn from(c: Command) -> Self {
        Reaction::IrcCmd(c)
    }
}

#[derive(Debug)]
pub enum ErrorReaction {
    Proceed,
    Quit(Option<Cow<'static, str>>),
}
