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
use super::irc_send::OutboxPort;
use super::irc_send::push_to_outbox;
use super::parse_msg_to_nick;
use super::reaction::LibReaction;
use crossbeam_utils;
use irc::client::prelude as aatxe;
use irc::client::prelude::Server as AatxeServer;
use irc::proto::Message;
use itertools::Itertools;
use smallvec::SmallVec;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::cmp;
use std::fmt::Display;
use std::iter;
use std::sync::Arc;
use util;

const UPDATE_MSG_PREFIX_STR: &'static str = "!!! UPDATE MESSAGE PREFIX !!!";

impl State {
    fn compose_msg<S1, S2>(
        &self,
        target: MsgTarget,
        addressee: S1,
        msg: S2,
    ) -> Result<Option<LibReaction<Message>>>
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

        let mut wrapped_msg = SmallVec::<[_; 1]>::new();

        for input_line in final_msg.lines() {
            wrap_msg(self, target, input_line, |output_line| {
                wrapped_msg.push(LibReaction::RawMsg(
                    aatxe::Command::PRIVMSG(
                        target.0.to_owned(),
                        output_line.to_owned(),
                    ).into(),
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
        target: MsgTarget,
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
            match self.compose_msg(target, addressee.borrow(), msg)? {
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

    fn prefix_len(&self) -> Result<usize> {
        Ok(self.read_msg_prefix()?.len())
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
    target: &str,
    msg: String,
    reaction: Reaction,
) -> Result<Option<LibReaction<Message>>> {
    let (reply_target, reply_addressee) = if target == state.nick()? {
        (MsgTarget(prefix.parse().nick.unwrap()), "")
    } else {
        (MsgTarget(target), prefix.parse().nick.unwrap_or(""))
    };

    match reaction {
        Reaction::None => Ok(None),
        Reaction::Msg(s) => state.compose_msg(reply_target, "", &s),
        Reaction::Msgs(a) => state.compose_msgs(reply_target, "", a.iter()),
        Reaction::Reply(s) => state.compose_msg(reply_target, reply_addressee, &s),
        Reaction::Replies(a) => state.compose_msgs(reply_target, reply_addressee, a.iter()),
        Reaction::RawMsg(s) => Ok(Some(LibReaction::RawMsg(s.parse()?))),
        Reaction::Quit(msg) => Ok(Some(mk_quit(msg))),
    }
}

fn handle_bot_command(
    state: &Arc<State>,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
) -> Option<LibReaction<Message>> {
    let reaction = (|| {
        let cmd_ln = parse_msg_to_nick(&msg, MsgTarget(&target), &state.nick()?)
            .expect("`handle_bot_command` shouldn't have been called!");

        debug_assert!(!cmd_ln.trim().is_empty());

        let mut cmd_name_and_args = cmd_ln.splitn(2, char::is_whitespace);
        let cmd_name = cmd_name_and_args.next().unwrap_or("");
        let cmd_args = cmd_name_and_args.next().unwrap_or("");

        Ok(bot_command_reaction(
            state,
            prefix.parse(),
            MsgTarget(&target),
            cmd_name,
            cmd_args,
        ))
    })();

    match reaction.and_then(|reaction| {
        handle_reaction(state, prefix, &target, msg, reaction)
    }) {
        Ok(r) => r,
        Err(e) => Some(LibReaction::RawMsg(
            aatxe::Command::PRIVMSG(
                target,
                format!(
                    "Encountered error while trying to handle command: {}",
                    e
                ),
            ).into(),
        )),
    }
}

fn run_bot_command(
    state: &State,
    metadata: MsgMetadata,
    &BotCommand {
        ref name,
        ref provider,
        ref auth_lvl,
        ref handler,
        ref usage_yaml,
        usage_str: _,
        help_msg: _,
    }: &BotCommand,
    cmd_args: &str
) -> BotCmdResult
{
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
            match util::run_handler(
                "command",
                name.clone(),
                || handler.run(state, &metadata, &arg),
            ) {
                Ok(r) => r,
                Err(e) => BotCmdResult::LibErr(e),
            }
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

pub fn mk_quit<'a>(msg: Option<Cow<'a, str>>) -> LibReaction<Message> {
    lazy_static! {
        static ref DEFAULT_QUIT_MSG: String = format!(
            "Built with <{}> v{}",
            env!("CARGO_PKG_HOMEPAGE"),
            env!("CARGO_PKG_VERSION")
        );
    }

    let quit = aatxe::Command::QUIT(msg.map(Cow::into_owned).or_else(
        || Some(DEFAULT_QUIT_MSG.clone()),
    )).into();

    LibReaction::RawMsg(quit)
}

pub(super) fn handle_msg<'xbs, 'xbsr>(
    state: &Arc<State>,
    crossbeam_scope: &'xbsr crossbeam_utils::scoped::Scope<'xbs>,
    server_id: ServerId,
    outbox: &OutboxPort,
    input_msg: Message,
) -> Result<()>
where
    'xbs: 'xbsr,
{
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
            push_to_outbox(outbox, server_id, handle_004(state)?);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_privmsg<'xbs, 'xbsr>(
    state: &Arc<State>,
    crossbeam_scope: &'xbsr crossbeam_utils::scoped::Scope<'xbs>,
    server_id: ServerId,
    outbox: &OutboxPort,
    prefix: OwningMsgPrefix,
    target: String,
    msg: String,
) -> Result<()>
where
    'xbs: 'xbsr,
{
    trace!(
        "[{}] Handling PRIVMSG: {:?}",
        state.server_socket_addr_dbg_string(server_id),
        msg
    );

    if !is_msg_to_nick(MsgTarget(&target), &msg, &state.nick()?) {
        return Ok(());
    }

    if parse_msg_to_nick(&msg, MsgTarget(&target), &state.nick()?)
        .unwrap()
        .is_empty()
    {
        // TODO: Use a trigger for this.
        push_to_outbox(
            &outbox,
            server_id,
            handle_reaction(state, prefix, &target, msg, Reaction::Reply("Yes?".into()))?,
        );

        Ok(())
    } else if prefix.parse().nick == Some(&target) && msg.trim() == UPDATE_MSG_PREFIX_STR {
        update_prefix_info(state, &prefix.parse())
    } else {
        // This could take a while or panic, so do it in a new thread.

        // These are cheap to clone, supposedly.
        let state = state.clone();
        let outbox = outbox.clone();

        let thread_spawn_result = crossbeam_scope.builder().spawn(move || {
            let lib_reaction = handle_bot_command(&state, prefix, target, msg);

            push_to_outbox(&outbox, server_id, lib_reaction);
        });

        Ok(())
    }
}

fn update_prefix_info(state: &State, prefix: &MsgPrefix) -> Result<()> {
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

fn handle_004(state: &State) -> Result<LibReaction<Message>> {
    // The server has finished sending the protocol-mandated welcome messages.

    send_msg_prefix_update_request(state)
}

// TODO: Run `send_msg_prefix_update_request` periodically.
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
