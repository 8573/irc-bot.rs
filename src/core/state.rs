use super::config;
use super::irc_msgs::OwningMsgPrefix;
use super::BotCommand;
use super::ErrorKind;
use super::MsgPrefix;
use super::Result;
use super::Server;
use super::ServerId;
use super::State;
use rand::StdRng;
use std::borrow::Cow;
use std::path::Path;
use std::sync::MutexGuard;
use std::sync::RwLockReadGuard;

impl State {
    pub fn nick(&self, server_id: ServerId) -> Result<String> {
        self.read_msg_prefix(server_id)?
            .parse()
            .nick
            .ok_or(ErrorKind::NicknameUnknown.into())
            .map(ToOwned::to_owned)
    }

    pub fn module_data_path(&self) -> Result<&Path> {
        Ok(self.module_data_path.as_ref())
    }

    pub fn command(&self, name: &str) -> Result<Option<&BotCommand>> {
        Ok(self.commands.get(name))
    }

    pub fn command_names(&self) -> Result<Vec<Cow<'static, str>>> {
        Ok(self.commands.keys().cloned().collect())
    }

    pub fn have_admin(
        &self,
        MsgPrefix {
            nick: nick_1,
            user: user_1,
            host: host_1,
        }: MsgPrefix,
    ) -> Result<bool> {
        Ok(self.config.admins.iter().any(
            |&config::Admin {
                 nick: ref nick_2,
                 user: ref user_2,
                 host: ref host_2,
             }| {
                check_admin_cred(nick_1, nick_2) && check_admin_cred(user_1, user_2)
                    && check_admin_cred(host_1, host_2)
            },
        ))
    }

    // TODO: This is server-specific.
    pub(super) fn read_msg_prefix(
        &self,
        _server_id: ServerId,
    ) -> Result<RwLockReadGuard<OwningMsgPrefix>> {
        self.msg_prefix
            .read()
            .map_err(|_| ErrorKind::LockPoisoned("stored message prefix".into()).into())
    }

    pub(super) fn read_server(
        &self,
        server_id: ServerId,
    ) -> Result<Option<RwLockReadGuard<Server>>> {
        match self.servers.get(&server_id) {
            Some(lock) => match lock.read() {
                Ok(guard) => Ok(Some(guard)),
                Err(_) => Err(ErrorKind::LockPoisoned(
                    format!("server {}", server_id.uuid.hyphenated()).into(),
                ).into()),
            },
            None => Ok(None),
        }
    }

    /// Allows access to a random number generator that's stored centrally, to avoid the cost of
    /// repeatedly initializing one.
    pub fn rng(&self) -> Result<MutexGuard<StdRng>> {
        self.rng.lock().map_err(|_| {
            ErrorKind::LockPoisoned("the central random number generator".into()).into()
        })
    }

    /// Returns a string identifying the server for debug purposes.
    ///
    /// TODO: This should return something less allocate-y.
    pub(super) fn server_socket_addr_dbg_string(&self, server_id: ServerId) -> String {
        let uuid = server_id.uuid.hyphenated();

        match self.read_server(server_id) {
            Ok(Some(s)) => s.socket_addr_string.clone(),
            Ok(None) => format!("<unknown server {} (not found)>", uuid),
            Err(e) => format!("<unknown server {} ({})>", uuid, e),
        }
    }
}

/// Check a field of a (nick, user, host) triple representing some user (the "candidate") against
/// the corresponding field of a like triple representing an authorized administrator of the bot
/// (the "control"). Returns whether the given candidate field matches the control.
fn check_admin_cred(candidate: Option<&str>, control: &Option<String>) -> bool {
    match (candidate, control) {
        (Some(cdt), &Some(ref ctl)) => {
            // If a field is set in both candidate and control, the values must be equal.
            cdt == ctl
        }
        (_, &None) => {
            // All candidates match against a field that is unset in the control record.
            true
        }
        (None, &Some(_)) => {
            // A candidate does not match if it lacks a field that is set in the control record.
            false
        }
    }
}
