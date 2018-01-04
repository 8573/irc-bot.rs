use core::Result;
use core::Server;
use core::State;
use irc::proto::Message;
use std::sync::Arc;

pub(super) fn send_main(state: Arc<State>, server: Server, thread_label: &str) -> Result<()> {
    // TODO

    Ok(())
}

/// All server-bound messages are to be passed through this function, which may modify them, and
/// may prevent a message from being sent by returning `None`.
pub fn process_outgoing_msg(_state: &State, msg: Message) -> Option<Message> {
    if true {
        debug!(" Sending message: {:?}", msg.to_string());
        Some(msg)
    } else {
        debug!("Dropping message: {:?}", msg.to_string());
        None
    }
}
