use super::State;
use irc::client::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MsgTarget<'a>(pub &'a str);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MsgPrefix<'a> {
    pub nick: Option<&'a str>,
    pub user: Option<&'a str>,
    pub host: Option<&'a str>,
}

#[derive(Debug)]
pub struct MsgMetadata<'a> {
    pub target: MsgTarget<'a>,
    pub prefix: MsgPrefix<'a>,
}

fn is_msg_to_nick(state: &State, MsgTarget(target): MsgTarget, msg: &str, nick: &str) -> bool {
    target == nick || msg == nick ||
    (msg.starts_with(nick) &&
     (msg.find(|c: char| {
                   state
                       .chars_indicating_msg_is_addressed_to_nick
                       .contains(&c)
               }) == Some(nick.len())))
}

fn user_msg(cmd: &Command) -> Option<(MsgTarget, &String)> {
    match cmd {
        &Command::PRIVMSG(ref target, ref msg) |
        &Command::NOTICE(ref target, ref msg) => Some((MsgTarget(target), msg)),
        _ => None,
    }
}

pub fn parse_msg_to_nick<'c>(state: &State,
                             cmd: &'c Command,
                             nick: &str)
                             -> Option<(MsgTarget<'c>, &'c str)> {
    user_msg(cmd).and_then(|(target, msg)| if is_msg_to_nick(state, target, msg, nick) {
                               Some((target,
                                     msg.trim_left_matches(nick)
                                         .trim_left_matches(|c: char| {
                                                                state
                            .chars_indicating_msg_is_addressed_to_nick
                            .contains(&c)
                                                            })
                                         .trim()))
                           } else {
                               None
                           })
}

pub fn parse_prefix(prefix: &Option<String>) -> MsgPrefix {
    let prefix = match prefix {
        &Some(ref s) => s,
        &None => return MsgPrefix::default(),
    };
    let mut iter = prefix.rsplitn(2, '@');
    let host = iter.next();
    let mut iter = iter.next().unwrap_or("").splitn(2, '!');
    let nick = iter.next();
    let user = iter.next();
    MsgPrefix {
        nick: nick,
        user: user,
        host: host,
    }
}
