use std::borrow::Cow;

#[derive(Debug)]
pub enum Reaction {
    None,
    Msg(Cow<'static, str>),
    Msgs(Cow<'static, [Cow<'static, str>]>),
    Reply(Cow<'static, str>),
    Replies(Cow<'static, [Cow<'static, str>]>),
    RawMsg(Cow<'static, str>),
    BotCmd(Cow<'static, str>),
    Quit(Option<Cow<'static, str>>),
}

#[derive(Debug)]
pub enum ErrorReaction {
    Proceed,
    Quit(Option<Cow<'static, str>>),
}
