use super::SessionId;
use irc::Message;

pub enum Action<Msg>
where
    Msg: Message,
{
    /// Take no action.
    None,

    /// Send a message like `Reaction::RawMsg`, in a specified session.
    RawMsg { session_id: SessionId, message: Msg },
}
