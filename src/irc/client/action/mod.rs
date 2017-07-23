use super::SessionId;
use irc::Message;

pub enum Action {
    /// Take no action.
    None,

    /// Send a message like `Reaction::RawMsg`, in a specified session.
    RawMsg {
        session: SessionId,
        message: Message,
    },
}
