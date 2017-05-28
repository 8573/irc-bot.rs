use super::BotCmdAuthLvl;
use super::BotCmdResult;
use super::BotCommand;
use super::ErrorKind;
use super::MsgMetadata;
use super::MsgPrefix;
use super::MsgTarget;
use super::Reaction;
use super::Result;
use super::State;
use super::parse_msg_to_nick;
use super::parse_prefix;
use irc::client::prelude::*;
use itertools::Itertools;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::cmp;
use std::fmt::Display;
use std::io;

const UPDATE_MSG_PREFIX_STR: &'static str = "!!! UPDATE MESSAGE PREFIX !!!";

impl<'server, 'modl> State<'server, 'modl> {
    pub fn say<S1, S2>(&self, MsgTarget(target): MsgTarget, addressee: S1, msg: S2) -> Result<()>
        where S1: Borrow<str>,
              S2: Display
    {
        let final_msg = format!(
            "{}{}{}",
            addressee.borrow(),
            if addressee.borrow().is_empty() {
                ""
            } else {
                &self.addressee_suffix
            },
            msg,
        );
        info!("Sending message to {:?}: {:?}", target, final_msg);
        wrap_msg(self, target, &final_msg, |line| {
            self.server
                .send_privmsg(target, line)
                .map_err(Into::into)
        })
    }
}

fn wrap_msg<F>(state: &State, target: &str, msg: &str, mut f: F) -> Result<()>
    where F: FnMut(&str) -> Result<()>
{
    // :nick!user@host PRIVMSG target :message
    // :nick!user@host NOTICE target :message
    let raw_len_limit = 512;
    let punctuation_len = {
        let line_terminator_len = 2;
        let spaces = 3;
        let colons = 2;
        colons + spaces + line_terminator_len
    };
    let prefix_len = state.msg_prefix_string.len();
    let cmd_len = "PRIVMSG".len();
    let metadata_len = prefix_len + cmd_len + target.len() + punctuation_len;
    let msg_len_limit = raw_len_limit - metadata_len;

    if msg.len() < msg_len_limit {
        return f(msg);
    }

    let mut split_end_idx = 0;

    let lines = msg.match_indices(char::is_whitespace)
        .peekable()
        .batching(|mut iter| {
            debug_assert!(msg.len() >= msg_len_limit);

            let split_start_idx = split_end_idx;

            if split_start_idx >= msg.len() {
                return None;
            }

            while let Some(&(next_space_idx, _)) = iter.peek() {
                if msg[split_start_idx..next_space_idx].len() < msg_len_limit {
                    split_end_idx = next_space_idx;
                    iter.next();
                } else {
                    break;
                }
            }

            if iter.peek().is_none() {
                split_end_idx = msg.len()
            } else if split_end_idx <= split_start_idx {
                split_end_idx = cmp::min(split_start_idx + msg_len_limit, msg.len())
            }

            Some(msg[split_start_idx..split_end_idx].trim())
        });

    for line in lines {
        f(line)?
    }

    Ok(())
}

fn handle_reaction(state: &State, msg_md: &MsgMetadata, reaction: Reaction) -> Result<()> {
    let &MsgMetadata {
             target,
             prefix: MsgPrefix { nick, .. },
         } = msg_md;

    let (reply_target, reply_addressee) = if target.0 == state.nick() {
        (MsgTarget(nick.unwrap()), "")
    } else {
        (target, nick.unwrap_or(""))
    };

    match reaction {
        Reaction::None => Ok(()),
        Reaction::Msg(s) => state.say(reply_target, "", &s),
        Reaction::Msgs(a) => {
            for s in a.iter() {
                state.say(reply_target, "", &s)?
            }
            Ok(())
        }
        Reaction::Reply(s) => state.say(reply_target, reply_addressee, &s),
        Reaction::Replies(a) => {
            for s in a.iter() {
                state.say(reply_target, reply_addressee, &s)?
            }
            Ok(())
        }
        Reaction::IrcCmd(c) => {
            match state.server.send(c) {
                Ok(()) => Ok(()),
                Err(e) => bail!(e),
            }
        }
        Reaction::BotCmd(cmd_ln) => handle_bot_command(state, msg_md, cmd_ln),
        Reaction::Quit(msg) => bail!(ErrorKind::ModuleRequestedQuit(msg)),
    }
}

fn handle_bot_command<C>(state: &State, msg_md: &MsgMetadata, command_line: C) -> Result<()>
    where C: Borrow<str>
{
    let cmd_ln = command_line.borrow();

    debug_assert!(!cmd_ln.trim().is_empty());

    let mut cmd_name_and_args = cmd_ln.splitn(2, char::is_whitespace);
    let cmd_name = cmd_name_and_args.next().unwrap_or("");
    let cmd_args = cmd_name_and_args.next().unwrap_or("");

    handle_reaction(state,
                    msg_md,
                    bot_command_reaction(state, msg_md, cmd_name, cmd_args))
}

    fn run_bot_command(state: &State, msg_md: &MsgMetadata, &BotCommand {
                 ref name,
                 ref provider,
                 ref auth_lvl,
                 ref handler,
                 usage: _,
                 help_msg: _,
}: &BotCommand, cmd_args: &str) -> BotCmdResult{

    let user_authorized = match auth_lvl {
        &BotCmdAuthLvl::Public => Ok(true),
        &BotCmdAuthLvl::Owner => state.have_owner(msg_md.prefix),
    };

    let result = match user_authorized {
        Ok(true) => handler.run(state, msg_md, cmd_args),
        Ok(false) => BotCmdResult::Unauthorized,
        Err(e) => BotCmdResult::LibErr(e),
    };

    match result {
        BotCmdResult::Ok(Reaction::Quit(ref s)) if *auth_lvl != BotCmdAuthLvl::Owner => {
            BotCmdResult::BotErrMsg(format!("Only commands at authorization level \
                                             {auth_lvl_owner:?} may tell the bot to quit, but \
                                             the command {cmd_name:?} from module \
                                             {provider_name:?}, at authorization level \
                                             {cmd_auth_lvl:?}, has told the bot to quit with \
                                             quit message {quit_msg:?}.",
                                            auth_lvl_owner = BotCmdAuthLvl::Owner,
                                            cmd_name = name,
                                            provider_name = provider.name,
                                            cmd_auth_lvl = auth_lvl,
                                            quit_msg = s)
                                            .into())
        }
        r => r,
    }
}

fn bot_command_reaction(state: &State,
                        msg_md: &MsgMetadata,
                        cmd_name: &str,
                        cmd_args: &str)
                        -> Reaction {
    let cmd = match state.commands.get(cmd_name) {
        Some(c) => c,
        None => {
            return Reaction::Reply(format!("Unknown command {:?}; apologies.", cmd_name).into())
        }
    };

    let &BotCommand {
             ref name,
             ref usage,
             ..
         } = cmd;

    let cmd_result = match run_bot_command(state, msg_md, cmd, cmd_args) {
        BotCmdResult::Ok(r) => Ok(r),
        BotCmdResult::Unauthorized => {
            Err(format!("My apologies, but you do not appear to have sufficient authority to use \
                         my {:?} command.",
                        name))
        }
        BotCmdResult::SyntaxErr => Err(format!("Syntax: {} {}", name, usage)),
        BotCmdResult::ArgMissing(arg_name) => {
            Err(format!("Syntax error: For command {:?}, the argument {:?} is required, but it \
                         was not given.",
                        name,
                        arg_name))
        }
        BotCmdResult::ArgMissing1To1(arg_name) => {
            Err(format!("Syntax error: When command {:?} is used outside of a channel, the \
                         argument {:?} is required, but it was not given.",
                        name,
                        arg_name))
        }
        BotCmdResult::LibErr(e) => Err(format!("Error: {}", e)),
        BotCmdResult::UserErrMsg(s) => Err(format!("User error: {}", s)),
        BotCmdResult::BotErrMsg(s) => Err(format!("Internal error: {}", s)),
    };

    match cmd_result {
        Ok(r) => r,
        Err(s) => Reaction::Reply(s.into()),
    }
}

pub fn quit<'a>(state: &State, msg: Option<Cow<'a, str>>) {
    let default_quit_msg = format!("<{}> v{}",
                                   env!("CARGO_PKG_HOMEPAGE"),
                                   env!("CARGO_PKG_VERSION"));

    info!("Quitting. Quit message: {:?}.", msg);

    state
        .server
        .send_quit(msg.unwrap_or(default_quit_msg.into()).as_ref())
        .unwrap_or_else(|err| error!("Error while quitting: {:?}", err))
}

pub fn handle_msg(state: &mut State, input_msg: io::Result<Message>) -> Result<()> {
    let raw_msg = match input_msg {
        Ok(m) => m,
        Err(e) => bail!(e),
    };

    debug!("{:?}", raw_msg);

    (match raw_msg.command {
         Command::PRIVMSG(..) => handle_privmsg,
         Command::NOTICE(..) => ignore_msg,
         Command::Response(Response::RPL_ENDOFMOTD, _, _) => handle_end_of_motd,
         _ => ignore_msg,
     })(state, raw_msg)
}

fn handle_privmsg(state: &mut State, raw_msg: Message) -> Result<()> {
    let Message {
        tags: _,
        ref prefix,
        ref command,
    } = raw_msg;

    let (target, msg) = match parse_msg_to_nick(state, command, state.nick()) {
        Some((t, m)) => (t, m),
        None => return Ok(()),
    };

    info!("{:?}", raw_msg);

    let msg_md = MsgMetadata {
        target: target,
        prefix: parse_prefix(prefix),
    };

    if msg.is_empty() {
        handle_reaction(state, &msg_md, Reaction::Reply("Yes?".into()))
    } else if msg_md.prefix.nick == Some(target.0) && msg == UPDATE_MSG_PREFIX_STR {
        if let Some(s) = prefix.to_owned() {
            info!("Setting stored message prefix to {:?}", s);
            state.msg_prefix_string = s;
            Ok(())
        } else {
            Err(ErrorKind::MsgPrefixUpdateRequestedButPrefixMissing.into())
        }
    } else {
        handle_bot_command(state, &msg_md, msg)
    }
}

fn handle_end_of_motd(state: &mut State, _: Message) -> Result<()> {
    state.say(MsgTarget(state.nick()), state.nick(), UPDATE_MSG_PREFIX_STR)
}

fn ignore_msg(_: &mut State, _: Message) -> Result<()> {
    Ok(())
}
