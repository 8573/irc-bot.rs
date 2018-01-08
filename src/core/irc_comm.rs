use super::BotCmdAuthLvl;
use super::BotCmdResult;
use super::BotCommand;
use super::ErrorKind;
use super::MsgMetadata;
use super::MsgPrefix;
use super::MsgTarget;
use super::Reaction;
use super::Result;
use super::ServerId;
use super::State;
use super::bot_cmd;
use super::irc_msgs::OwningMsgPrefix;
use super::irc_msgs::PrivMsg;
use super::irc_msgs::is_msg_to_nick;
use super::irc_msgs::parse_prefix;
use super::irc_msgs::parse_privmsg;
use super::irc_send;
use super::parse_msg_to_nick;
use super::reaction::LibReaction;
use crossbeam;
use irc::client::prelude as aatxe;
use irc::client::prelude::Server as AatxeServer;
use irc::proto::Message;
use itertools::Itertools;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::cmp;
use std::fmt::Display;
use std::iter;
use std::sync::Arc;

const UPDATE_MSG_PREFIX_STR: &'static str = "!!! UPDATE MESSAGE PREFIX !!!";

impl State {
    fn say<S1, S2>(&self, target: MsgTarget, addressee: S1, msg: S2) -> Result<LibReaction<Message>>
    where
        S1: Borrow<str>,
        S2: Display,
    {
        let final_msg =
            format!(
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
        let mut wrapped_msg = vec![];
        wrap_msg(self, target, &final_msg, |line| {
            wrapped_msg.push(LibReaction::RawMsg(
                aatxe::Command::PRIVMSG(
                    target.0.to_owned(),
                    line.to_owned(),
                ).into(),
            ));
            Ok(())
        })?;
        // TODO: optimize for case where no wrapping, and thus no `Vec`, is needed.
        Ok(LibReaction::Multi(wrapped_msg))
    }

    fn prefix_len(&self) -> Result<usize> {
        Ok(self.msg_prefix.read().len())
    }
}

fn wrap_msg<F>(state: &State, MsgTarget(target): MsgTarget, msg: &str, mut f: F) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
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
    let cmd_len = "PRIVMSG".len();
    let metadata_len = state.prefix_len()? + cmd_len + target.len() + punctuation_len;
    let msg_len_limit = raw_len_limit - metadata_len;

    if msg.len() < msg_len_limit {
        return f(msg);
    }

    let mut split_end_idx = 0;

    let lines = msg.match_indices(char::is_whitespace).peekable().batching(
        |mut iter| {
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
        },
    );

    for line in lines {
        f(line)?
    }

    Ok(())
}

fn handle_reaction(
    state: &Arc<State>,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
    reaction: Reaction,
) -> Result<LibReaction<Message>> {
    let (reply_target, reply_addressee) = if target == state.nick()? {
        (MsgTarget(prefix.parse().nick.unwrap()), "")
    } else {
        (MsgTarget(&target), prefix.parse().nick.unwrap_or(""))
    };

    match reaction {
        Reaction::None => Ok(LibReaction::None),
        Reaction::Msg(s) => state.say(reply_target, "", &s),
        Reaction::Msgs(a) => {
            Ok(LibReaction::Multi(a.iter()
                .map(|s| state.say(reply_target, "", &s))
                .collect::<Result<_>>()?))
        }
        Reaction::Reply(s) => state.say(reply_target, reply_addressee, &s),
        Reaction::Replies(a) => {
            Ok(LibReaction::Multi(a.iter()
                .map(|s| state.say(reply_target, reply_addressee, &s))
                .collect::<Result<_>>()?))
        }
        Reaction::RawMsg(s) => Ok(LibReaction::RawMsg(s.parse()?)),
        Reaction::Quit(msg) => Ok(LibReaction::RawMsg(
            aatxe::Command::QUIT(msg.map(Into::into)).into(),
        )),
    }
}

fn handle_bot_command(
    state: &Arc<State>,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
) -> Result<LibReaction<Message>> {
    let reaction = {
        let cmd_ln = parse_msg_to_nick(&msg, MsgTarget(&target), &state.nick()?)
            .expect("`handle_bot_command` shouldn't have been called!");

        debug_assert!(!cmd_ln.trim().is_empty());

        let mut cmd_name_and_args = cmd_ln.splitn(2, char::is_whitespace);
        let cmd_name = cmd_name_and_args.next().unwrap_or("");
        let cmd_args = cmd_name_and_args.next().unwrap_or("");

        bot_command_reaction(
            state,
            prefix.parse(),
            MsgTarget(&target),
            cmd_name,
            cmd_args,
        )
    };

    handle_reaction(state, prefix, target, msg, reaction)
}

    fn run_bot_command(state: &State, metadata: MsgMetadata, &BotCommand {
                 ref name,
                 ref provider,
                 ref auth_lvl,
                 ref handler,
                 ref usage_yaml,
                 usage_str: _,
                 help_msg: _,
}: &BotCommand, cmd_args: &str) -> BotCmdResult{

    let user_authorized = match auth_lvl {
        &BotCmdAuthLvl::Public => Ok(true),
        &BotCmdAuthLvl::Admin => state.have_admin(metadata.prefix),
    };

    let arg = match bot_cmd::parse_arg(usage_yaml, cmd_args) {
        Ok(arg) => arg,
        Err(res) => return res,
    };

    let result = match user_authorized {
        Ok(true) => {
            debug!("Running bot command {:?} with arg: {:?}", name, arg);
            handler.run(state, &metadata, &arg)
        }
        Ok(false) => BotCmdResult::Unauthorized,
        Err(e) => BotCmdResult::LibErr(e),
    };

    // TODO: Filter `QUIT`s in `irc_send` instead, and check `Reaction::RawMsg`s as well.
    match result {
        BotCmdResult::Ok(Reaction::Quit(ref s)) if *auth_lvl != BotCmdAuthLvl::Admin => {
            BotCmdResult::BotErrMsg(
                format!(
                    "Only commands at authorization level {auth_lvl_owner:?} may tell the bot to \
                     quit, but the command {cmd_name:?} from module {provider_name:?}, at \
                     authorization level {cmd_auth_lvl:?}, has told the bot to quit with quit \
                     message {quit_msg:?}.",
                    auth_lvl_owner = BotCmdAuthLvl::Admin,
                    cmd_name = name,
                    provider_name = provider.name,
                    cmd_auth_lvl = auth_lvl,
                    quit_msg = s
                ).into(),
            )
        }
        r => r,
    }
}

fn bot_command_reaction(
    state: &Arc<State>,
    prefix: MsgPrefix,
    target: MsgTarget,
    cmd_name: &str,
    cmd_args: &str,
) -> Reaction {
    let metadata = MsgMetadata { prefix, target };

    let cmd = match state.commands.get(cmd_name) {
        Some(c) => c,
        None => {
            return Reaction::Reply(format!("Unknown command {:?}; apologies.", cmd_name).into())
        }
    };

    let &BotCommand {
        ref name,
        ref usage_str,
        ..
    } = cmd;

    let cmd_result = match run_bot_command(state, metadata, cmd, cmd_args) {
        BotCmdResult::Ok(r) => Ok(r),
        BotCmdResult::Unauthorized => {
            Err(format!(
                "My apologies, but you do not appear to have sufficient \
                 authority to use my {:?} command.",
                name
            ))
        }
        BotCmdResult::SyntaxErr => Err(format!("Syntax: {} {}", name, usage_str)),
        BotCmdResult::ArgMissing(arg_name) => {
            Err(format!(
                "Syntax error: For command {:?}, the argument {:?} is \
                 required, but it was not given.",
                name,
                arg_name
            ))
        }
        BotCmdResult::ArgMissing1To1(arg_name) => {
            Err(format!(
                "Syntax error: When command {:?} is used outside of a \
                 channel, the argument {:?} is required, but it was not \
                 given.",
                name,
                arg_name
            ))
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

pub fn quit<'a>(state: &State, msg: Option<Cow<'a, str>>) -> LibReaction<Message> {
    let default_quit_msg = format!(
        "<{}> v{}",
        env!("CARGO_PKG_HOMEPAGE"),
        env!("CARGO_PKG_VERSION")
    );

    let msg: Option<&str> = msg.as_ref().map(Borrow::borrow);

    info!("Quitting. Quit message: {:?}.", msg);

    let quit = match format!("QUIT :{}", msg.unwrap_or(&default_quit_msg))
        .parse()
        .map_err(Into::into) {
        Ok(m) => m,
        Err(e) => {
            (state.error_handler)(&e);
            error!("Failed to construct quit message.");
            return LibReaction::None;
        }
    };

    LibReaction::RawMsg(quit)
}

pub(super) fn handle_msg<'xbs, 'xbsr>(
    state: &Arc<State>,
    crossbeam_scope: &'xbsr crossbeam::Scope<'xbs>,
    server_id: ServerId,
    outbox: &irc_send::OutboxPort,
    input_msg: Message,
) -> Result<LibReaction<Message>>
where
    'xbs: 'xbsr,
{
    trace!(
        "[{}] Received {:?}",
        state.server_socket_addr_string(server_id),
        input_msg.to_string().trim_right_matches("\r\n")
    );

    match input_msg {
        Message {
            command: aatxe::Command::PRIVMSG(target, msg),
            prefix,
            ..
        } => {
            handle_privmsg(
                state,
                crossbeam_scope,
                server_id,
                outbox,
                OwningMsgPrefix::from_string(prefix.unwrap_or_default()),
                target,
                msg,
            )
        }
        Message { command: aatxe::Command::Response(aatxe::Response::RPL_MYINFO, ..), .. } => {
            handle_004(state)
        }
        _ => Ok(LibReaction::None),
    }
}

fn handle_privmsg<'xbs, 'xbsr>(
    state: &Arc<State>,
    crossbeam_scope: &'xbsr crossbeam::Scope<'xbs>,
    server_id: ServerId,
    outbox: &irc_send::OutboxPort,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
) -> Result<LibReaction<Message>>
where
    'xbs: 'xbsr,
{
    trace!(
        "[{}] Handling PRIVMSG: {:?}",
        state.server_socket_addr_string(server_id),
        msg
    );

    if !is_msg_to_nick(MsgTarget(&target), &msg, &state.nick()?) {
        return Ok(LibReaction::None);
    }

    if parse_msg_to_nick(&msg, MsgTarget(&target), &state.nick()?)
        .unwrap()
        .is_empty()
    {
        // TODO: Use a trigger for this.
        handle_reaction(state, prefix, target, msg, Reaction::Reply("Yes?".into()))
    } else if prefix.parse().nick == Some(&target) && msg.trim() == UPDATE_MSG_PREFIX_STR {
        update_prefix_info(state, &prefix.parse())
    } else {
        // This could take a while or panic, so do it in a new thread.

        // TODO: Add a command that specifically panics, to test panic catching.

        // These are cheap to clone, supposedly.
        let state = state.clone();
        let outbox = outbox.clone();

        let thread_spawn_result = crossbeam_scope.builder().spawn(move || {
            let lib_reaction = match handle_bot_command(&state, prefix, target, msg) {
                Ok(r) => r,
                Err(e) => {
                    // TODO: Inform the user invoking the command of the error.
                    error!("Error in command handling: {:?}", e);
                    return;
                }
            };

            irc_send::push_to_outbox(&outbox, server_id, lib_reaction);
        });

        Ok(LibReaction::None)
    }
}

fn update_prefix_info(state: &State, prefix: &MsgPrefix) -> Result<LibReaction<Message>> {
    debug!(
        "Updating stored message prefix information from received {:?}",
        prefix
    );

    state.msg_prefix.write().update_from(prefix);

    Ok(LibReaction::None)
}

fn handle_004(state: &State) -> Result<LibReaction<Message>> {
    // The server has finished sending the protocol-mandated welcome messages.

    send_msg_prefix_update_request(state)
}

fn send_msg_prefix_update_request(state: &State) -> Result<LibReaction<Message>> {
    Ok(LibReaction::RawMsg(
        aatxe::Command::PRIVMSG(
            state.nick()?.to_owned(),
            UPDATE_MSG_PREFIX_STR.to_owned(),
        ).into(),
    ))
}

fn connection_sequence(state: &State) -> Result<Vec<Message>> {
    Ok(vec![
        aatxe::Command::NICK(state.config.nickname.to_owned())
            .into(),
        aatxe::Command::USER(
            state.config.username.to_owned(),
            "8".to_owned(),
            state.config.realname.to_owned()
        ).into(),
    ])
}
