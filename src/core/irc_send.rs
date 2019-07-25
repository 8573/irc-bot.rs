use super::ErrorKind;
use super::LibReaction;
use super::ServerId;
use super::THREAD_NAME_FAIL;
use core::Error;
use core::Result;
use core::State;
use crossbeam_channel;
use irc::client::prelude as aatxe;
use irc::client::prelude::Client as AatxeClient;
use irc::proto::Message;
use std::sync::Arc;
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
        Err(crossbeam_channel::TrySendError::Disconnected(record)) => error!(
            "Outbox receiver disconnected!!! Could not send {record:?}",
            record = record
        ),
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
    for record in outbox_receiver.iter() {
        let OutboxRecord {
            server_id, output, ..
        } = match process_outgoing_msg(&state, thread_label, record) {
            Some(a) => a,
            None => continue,
        };

        let aatxe_clients = match state.aatxe_clients.read() {
            Ok(map) => map,
            Err(_) => {
                // TODO: This lock being poisoned is so grave that it deserves its own error kind.
                return Err(ErrorKind::LockPoisoned(
                    "the associative array of IRC connections".into(),
                )
                .into());
            }
        };

        let aatxe_client = match aatxe_clients.get(&server_id) {
            Some(client) => client.clone(),
            None => {
                warn!(
                    "Can't send to unknown server {server_id:?}. Discarding {output:?}.",
                    server_id = server_id,
                    output = output
                );
                continue;
            }
        };

        send_reaction(&state, &aatxe_client, thread_label, output)
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
    aatxe_client: &aatxe::IrcClient,
    thread_label: &str,
    reaction: LibReaction<Message>,
) {
    send_reaction_with_err_cb(state, aatxe_client, thread_label, reaction, |err| {
        let err_reaction = match state.handle_err_generic(err) {
            Some(r) => r,
            None => return,
        };

        send_reaction_with_err_cb(state, aatxe_client, thread_label, err_reaction, |err| {
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
    aatxe_client: &aatxe::IrcClient,
    thread_label: &str,
    reaction: LibReaction<Message>,
    err_cb: ErrCb,
) where
    ErrCb: Fn(Error) -> (),
{
    match reaction {
        LibReaction::RawMsg(msg) => match aatxe_client.send(msg) {
            Ok(()) => {}
            Err(e) => err_cb(e.into()),
        },
        LibReaction::Multi(reactions) => {
            for reaction in reactions {
                send_reaction(state, aatxe_client, thread_label, reaction)
            }
        }
    }
}
