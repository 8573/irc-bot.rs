use super::bot_cmd;
use super::irc_msgs::is_msg_to_nick;
use super::irc_msgs::OwningMsgPrefix;
use super::irc_send::push_to_outbox;
use super::irc_send::OutboxPort;
use super::parse_msg_to_nick;
use super::pkg_info;
use super::reaction::LibReaction;
use super::trigger;
use super::BotCmdResult;
use super::ErrorKind;
use super::MsgDest;
use super::MsgMetadata;
use super::MsgPrefix;
use super::Reaction;
use super::Result;
use super::ServerId;
use super::State;
use irc::client::prelude as aatxe;
use irc::proto::Message;
use itertools::Itertools;
use smallvec::SmallVec;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::cmp;
use std::fmt::Display;
use std::sync::Arc;
use std::thread;

const UPDATE_MSG_PREFIX_STR: &'static str = "!!! UPDATE MESSAGE PREFIX !!!";

impl State {
    fn compose_msg<S1, S2>(
        &self,
        dest: MsgDest,
        addressee: S1,
        msg: S2,
    ) -> Result<Option<LibReaction<Message>>>
    where
        S1: Borrow<str>,
        S2: Display,
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

        info!("Sending message to {:?}: {:?}", dest, final_msg);

        let mut wrapped_msg = SmallVec::<[_; 1]>::new();

        for input_line in final_msg.lines() {
            wrap_msg(self, dest, input_line, |output_line| {
                wrapped_msg.push(LibReaction::RawMsg(
                    aatxe::Command::PRIVMSG(dest.target.to_owned(), output_line.to_owned()).into(),
                ));
                Ok(())
            })?;
        }

        match wrapped_msg.len() {
            0 => Ok(None),
            1 => Ok(Some(wrapped_msg.remove(0))),
            _ => Ok(Some(LibReaction::Multi(wrapped_msg.into_vec()))),
        }
    }

    fn compose_msgs<S1, S2, M>(
        &self,
        dest: MsgDest,
        addressee: S1,
        msgs: M,
    ) -> Result<Option<LibReaction<Message>>>
    where
        S1: Borrow<str>,
        S2: Display,
        M: IntoIterator<Item = S2>,
    {
        // Not `SmallVec`, because we're guessing that the caller expects multiple messages.
        let mut output = Vec::new();

        for msg in msgs {
            match self.compose_msg(dest, addressee.borrow(), msg)? {
                Some(m) => output.push(m),
                None => {}
            }
        }

        match output.len() {
            0 => Ok(None),
            1 => Ok(Some(output.remove(0))),
            _ => Ok(Some(LibReaction::Multi(output))),
        }
    }

    /// Given a message's metadata, returns a guess at the destination to which replies to the
    /// message should be sent.
    pub fn guess_reply_dest<'a>(
        &self,
        &MsgMetadata {
            dest: MsgDest { server_id, target },
            prefix:
                MsgPrefix {
                    nick,
                    user: _,
                    host: _,
                },
        }: &MsgMetadata<'a>,
    ) -> Result<MsgDest<'a>> {
        Ok(MsgDest {
            server_id,
            target: if target == self.nick(server_id)? {
                // The message was sent to the bot in one-to-one messaging, so replies should be
                // sent in one-to-one messaging to the sender.
                nick.ok_or(ErrorKind::ReceivedMsgHasBadPrefix)?
            } else {
                // The message was sent in a channel, so replies should be sent in the same
                // channel.
                target
            },
        })
    }

    // TODO: Use the more up-to-date nick info from `IrcClient::current_nickname` in `irc` 0.14.
    fn prefix_len(&self, server_id: ServerId) -> Result<usize> {
        Ok(self.read_msg_prefix(server_id)?.len())
    }

    /// Returns the maximum number of bytes that can be sent as the content of a single `PRIVMSG`
    /// to the specified destination.
    pub fn privmsg_content_max_len(&self, MsgDest { server_id, target }: MsgDest) -> Result<usize> {
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
        let metadata_len = self.prefix_len(server_id)? + cmd_len + target.len() + punctuation_len;
        Ok(raw_len_limit - metadata_len)
    }
}

fn wrap_msg<F>(state: &State, msg_dest: MsgDest, msg: &str, mut f: F) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    let msg_len_limit = state.privmsg_content_max_len(msg_dest)?;

    if msg.len() < msg_len_limit {
        return f(msg);
    }

    let mut split_end_idx = 0;

    let lines = msg.match_indices(char::is_whitespace)
        .peekable()
        .batching(|iter| {
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

fn handle_reaction(
    state: &Arc<State>,
    server_id: ServerId,
    prefix: OwningMsgPrefix,
    target: &str,
    reaction: Reaction,
    bot_nick: String,
) -> Result<Option<LibReaction<Message>>> {
    let (reply_target, reply_addressee) = if target == bot_nick {
        (prefix.parse().nick.unwrap(), "")
    } else {
        (target, prefix.parse().nick.unwrap_or(""))
    };

    let reply_dest = MsgDest {
        server_id,
        target: reply_target,
    };

    match reaction {
        Reaction::None => Ok(None),
        Reaction::Msg(s) => state.compose_msg(reply_dest, "", &s),
        Reaction::Msgs(a) => state.compose_msgs(reply_dest, "", a.iter()),
        Reaction::Reply(s) => state.compose_msg(reply_dest, reply_addressee, &s),
        Reaction::Replies(a) => state.compose_msgs(reply_dest, reply_addressee, a.iter()),
        Reaction::RawMsg(s) => Ok(Some(LibReaction::RawMsg(s.parse()?))),
        Reaction::Quit(msg) => Ok(Some(mk_quit(msg))),
    }
}

fn handle_bot_command_or_trigger(
    state: &Arc<State>,
    server_id: ServerId,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
    bot_nick: String,
) -> Option<LibReaction<Message>> {
    let reaction = (|| {
        let metadata = MsgMetadata {
            prefix: prefix.parse(),
            dest: MsgDest {
                server_id,
                target: &target,
            },
        };

        let cmd_ln = parse_msg_to_nick(&msg, metadata.dest.target, &bot_nick).unwrap_or("");

        let mut cmd_name_and_args = cmd_ln.splitn(2, char::is_whitespace);
        let cmd_name = cmd_name_and_args.next().unwrap_or("");
        let cmd_args = cmd_name_and_args.next().unwrap_or("").trim();

        if let Some(r) = bot_cmd::run(state, cmd_name, cmd_args, &metadata)? {
            Ok(bot_command_reaction(cmd_name, r))
        } else if let Some(r) = trigger::run_any_matching(state, cmd_ln, &metadata)? {
            Ok(bot_command_reaction("<trigger>", r))
        } else {
            Ok(Reaction::None)
        }
    })();

    match reaction
        .and_then(|reaction| handle_reaction(state, server_id, prefix, &target, reaction, bot_nick))
    {
        Ok(r) => r,
        Err(e) => Some(LibReaction::RawMsg(
            aatxe::Command::PRIVMSG(
                target,
                format!("Encountered error while trying to handle message: {}", e),
            ).into(),
        )),
    }
}

fn bot_command_reaction(cmd_name: &str, result: BotCmdResult) -> Reaction {
    let cmd_result = match result {
        BotCmdResult::Ok(r) => Ok(r),
        BotCmdResult::Unauthorized => Err(format!(
            "My apologies, but you do not appear to have sufficient \
             authority to use my {:?} command.",
            cmd_name
        ).into()),
        BotCmdResult::ParamUnauthorized(param_name) => Err(format!(
            "My apologies, but you do not appear to have sufficient \
             authority to use the parameter {:?} of my {:?} command.",
            param_name, cmd_name
        ).into()),
        BotCmdResult::SyntaxErr => Err("Syntax error. Try my `help` command.".into()),
        BotCmdResult::ArgMissing(arg_name) => Err(format!(
            "Syntax error: For command {:?}, the argument {:?} \
             is required, but it was not given.",
            cmd_name, arg_name
        ).into()),
        BotCmdResult::ArgMissing1To1(arg_name) => Err(format!(
            "Syntax error: When command {:?} is used \
             outside of a channel, the argument {:?} is \
             required, but it was not given.",
            cmd_name, arg_name
        ).into()),
        BotCmdResult::LibErr(e) => Err(format!("Error: {}", e).into()),
        BotCmdResult::UserErrMsg(s) => Err(format!("User error: {}", s).into()),
        BotCmdResult::BotErrMsg(s) => Err(format!("Internal error: {}", s).into()),
    };

    match cmd_result {
        Ok(r) => r,
        Err(s) => Reaction::Msg(s),
    }
}

pub fn mk_quit<'a>(msg: Option<Cow<'a, str>>) -> LibReaction<Message> {
    let quit = aatxe::Command::QUIT(
        msg.map(Cow::into_owned)
            .or_else(|| Some(pkg_info::BRIEF_CREDITS_STRING.clone())),
    ).into();

    LibReaction::RawMsg(quit)
}

pub(super) fn handle_msg(
    state: &Arc<State>,
    server_id: ServerId,
    outbox: &OutboxPort,
    input_msg: Message,
) -> Result<()> {
    trace!(
        "[{}] Received {:?}",
        state.server_socket_addr_dbg_string(server_id),
        input_msg.to_string().trim_right_matches("\r\n")
    );

    match input_msg {
        Message {
            command: aatxe::Command::PRIVMSG(target, msg),
            prefix,
            ..
        } => handle_privmsg(
            state,
            server_id,
            outbox,
            OwningMsgPrefix::from_string(prefix.unwrap_or_default()),
            target,
            msg,
        ),
        Message {
            command: aatxe::Command::Response(aatxe::Response::RPL_MYINFO, ..),
            ..
        } => {
            push_to_outbox(outbox, server_id, handle_004(state, server_id)?);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_privmsg(
    state: &Arc<State>,
    server_id: ServerId,
    outbox: &OutboxPort,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
) -> Result<()> {
    trace!(
        "[{}] Handling PRIVMSG: {:?}",
        state.server_socket_addr_dbg_string(server_id),
        msg
    );

    let bot_nick = state.nick(server_id)?;

    if !is_msg_to_nick(&target, &msg, &bot_nick) {
        return Ok(());
    }

    if prefix.parse().nick == Some(&target) && msg.trim() == UPDATE_MSG_PREFIX_STR {
        update_prefix_info(state, server_id, &prefix.parse())
    } else {
        // This could take a while or panic, so do it in a new thread.

        // These are cheap to clone, supposedly.
        let state = state.clone();
        let outbox = outbox.clone();

        let thread_spawn_result = thread::Builder::new().spawn(move || {
            let lib_reaction =
                handle_bot_command_or_trigger(&state, server_id, prefix, target, msg, bot_nick);

            push_to_outbox(&outbox, server_id, lib_reaction);
        });

        match thread_spawn_result {
            Ok(thread::JoinHandle { .. }) => Ok(()),
            Err(e) => Err(ErrorKind::ThreadSpawnFailure(e).into()),
        }
    }
}

fn update_prefix_info(state: &State, _server_id: ServerId, prefix: &MsgPrefix) -> Result<()> {
    debug!(
        "Updating stored message prefix information from received {:?}",
        prefix
    );

    match state.msg_prefix.write() {
        Ok(guard) => guard,
        Err(poisoned_guard) => {
            // The lock was poisoned, you say? That's strange, unfortunate, and unlikely to be a
            // problem here, because we're just going to overwrite the contents anyway.
            warn!(
                "Stored message prefix was poisoned by thread panic! Discarding it, replacing it, \
                 and moving on."
            );
            poisoned_guard.into_inner()
        }
    }.update_from(prefix);

    Ok(())
}

fn handle_004(state: &State, server_id: ServerId) -> Result<LibReaction<Message>> {
    // The server has finished sending the protocol-mandated welcome messages.

    send_msg_prefix_update_request(state, server_id)
}

// TODO: Run `send_msg_prefix_update_request` periodically.
fn send_msg_prefix_update_request(
    state: &State,
    server_id: ServerId,
) -> Result<LibReaction<Message>> {
    Ok(LibReaction::RawMsg(
        aatxe::Command::PRIVMSG(
            state.nick(server_id)?.to_owned(),
            UPDATE_MSG_PREFIX_STR.to_owned(),
        ).into(),
    ))
}
