use irc::proto::Message;
use std::borrow::Cow;
use std::fmt;

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

/// Copied from `yak-irc`'s `Reaction`.
#[derive(Clone, Debug)]
pub enum LibReaction<Msg>
where
    Msg: Clone + fmt::Debug,
{
    /// No reaction.
    None,

    /// React by sending an IRC message. No line-wrapping or other formatting will be performed on
    /// the message, except that the message-terminating sequence of a carriage return character
    /// and a line feed character ("CR-LF") will be appended. If the message exceeds 512 octets in
    /// length (including the terminating CR-LF sequence, but excluding any IRCv3 message tags), it
    /// may be truncated to 512 octets.
    RawMsg(Msg),

    /// Return multiple reactions, which will be processed in the order given.
    Multi(Vec<LibReaction<Msg>>),
}
