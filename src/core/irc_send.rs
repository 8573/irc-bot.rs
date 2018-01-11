use super::LibReaction;
use super::ServerId;
use super::THREAD_NAME_FAIL;
use core::Error;
use core::Result;
use core::State;
use crossbeam_channel;
use irc::client::prelude as aatxe;
use irc::client::server::Server as AatxeServer;
use irc::proto::Message;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;

pub(super) const OUTBOX_SIZE: usize = 1024;

pub(super) type OutboxPort = crossbeam_channel::Sender<OutboxRecord>;

#[derive(Debug)]
pub(super) struct OutboxRecord {
    server_id: ServerId,
    output: LibReaction<Message>,
}

pub(super) fn push_to_outbox<O>(outbox_sender: &OutboxPort, server_id: ServerId, output: O)
where
    O: Into<Option<LibReaction<Message>>>,
{
    let output = match output.into() {
        Some(r) => r,
        None => return,
    };

    let result = outbox_sender.try_send(OutboxRecord { server_id, output });

    match result {
        Ok(()) => {}
        Err(crossbeam_channel::TrySendError::Full(record)) => {
            error!("Outbox full!!! Could not send {record:?}", record = record)
        }
        Err(crossbeam_channel::TrySendError::Disconnected(record)) => {
            error!(
                "Outbox receiver disconnected!!! Could not send {record:?}",
                record = record
            )
        }
    }
}

pub(super) fn send_main(
    state: Arc<State>,
    outbox_receiver: crossbeam_channel::Receiver<OutboxRecord>,
) -> Result<()> {
    let current_thread = thread::current();
    let thread_label = current_thread.name().expect(THREAD_NAME_FAIL);

    // [2018-01-08 - c74d] At least with `crossbeam_channel`'s MPSC queue implementation, this loop
    // will run until — and the sending thread will exit when — all receiving (and
    // command-handling, etc.) threads have exited. Not having to implement that myself is nice.
    for record in outbox_receiver {
        let OutboxRecord { server_id, output, .. } =
            match process_outgoing_msg(&state, thread_label, record) {
                Some(a) => a,
                None => continue,
            };

        let server_uuid = server_id.uuid.hyphenated();

        let aatxe_server = match state.servers.get(&server_id).map(RwLock::read) {
            Some(Ok(s)) => s.inner.clone(),
            Some(Err(_)) => {
                warn!(
                    "Declining to send to server {uuid} because its lock has been poisoned by \
                     thread panic! Discarding {output:?}.",
                    uuid = server_uuid,
                    output = output
                );
                continue;
            }
            None => {
                warn!(
                    "Can't send to unknown server {uuid}. Discarding {output:?}.",
                    uuid = server_uuid,
                    output = output
                );
                continue;
            }
        };

        send_reaction(&state, &aatxe_server, thread_label, output)
    }

    Ok(())
}

/// All server-bound messages are to be passed through this function, which may modify them, and
/// may prevent a message from being sent by returning `None`.
pub(super) fn process_outgoing_msg(
    _state: &State,
    _thread_label: &str,
    OutboxRecord { server_id, output }: OutboxRecord,
) -> Option<OutboxRecord> {
    // TODO: Deny sending a message if too many identical messages have been sent too recently in
    // the same channel/query.
    //
    // TODO: Deny sending a `QUIT` if the originating command lacks `Admin` authorization.
    if true {
        debug!("Sending {:?}", output);
        Some(OutboxRecord { server_id, output })
    } else {
        debug!("Dropping {:?}", output);
        None
    }
}

fn send_reaction(
    state: &State,
    server: &aatxe::IrcServer,
    thread_label: &str,
    reaction: LibReaction<Message>,
) {
    send_reaction_with_err_cb(state, server, thread_label, reaction, |err| {
        let err_reaction = match state.handle_err_generic(err) {
            Some(r) => r,
            None => return,
        };

        send_reaction_with_err_cb(state, server, thread_label, err_reaction, |err| {
            error!(
                "Encountered error {:?} while handling error; stopping error handling to avoid \
                 potential infinite recursion.",
                err
            )
        })
    })
}

fn send_reaction_with_err_cb<ErrCb>(
    state: &State,
    server: &aatxe::IrcServer,
    thread_label: &str,
    reaction: LibReaction<Message>,
    err_cb: ErrCb,
) where
    ErrCb: Fn(Error) -> (),
{
    match reaction {
        LibReaction::RawMsg(msg) => {
            match server.send(msg) {
                Ok(()) => {}
                Err(e) => err_cb(e.into()),
            }
        }
        LibReaction::Multi(reactions) => {
            for reaction in reactions {
                send_reaction(state, server, thread_label, reaction)
            }
        }
    }
}
