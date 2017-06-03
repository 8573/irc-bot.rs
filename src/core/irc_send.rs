use core::State;
use core::err::Error;
use futures::Sink;
use futures::sync::mpsc;
use pircolate;

/// All server-bound messages are to be passed through this function, which may modify them, and
/// may prevent a message from being sent by returning `None`.
pub fn process_outgoing_msg(state: &State, msg: pircolate::Message) -> Option<pircolate::Message> {
    if true {
        debug!(" Sending message: {:?}", msg.raw_message());
        Some(msg)
    } else {
        debug!("Dropping message: {:?}", msg.raw_message());
        None
    }
}
