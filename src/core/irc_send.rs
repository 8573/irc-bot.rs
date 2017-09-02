use core::State;
use irc::proto::Message;

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
