use super::Client;
use super::SessionId;
use irc::Message;

#[derive(Debug)]
pub struct MessageContext {
    // TODO: Make these fields `pub_restricted` once I get 1.18.
    pub session_id: SessionId,
}

impl MessageContext {
    pub fn session_id(&self) -> SessionId {
        self.session_id.clone()
    }
}
