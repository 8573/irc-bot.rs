use super::config;
use super::irc_msgs::OwningMsgPrefix;
use super::BotCommand;
use super::ErrorKind;
use super::MsgPrefix;
use super::Result;
use super::Server;
use super::ServerConfigIndex;
use super::ServerId;
use super::State;
use irc::client::prelude as aatxe;
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
                check_admin_cred(nick_1, nick_2)
                    && check_admin_cred(user_1, user_2)
                    && check_admin_cred(host_1, host_2)
            },
        ))
    }

    // TODO: This is server-specific.
    // TODO: This should be named `read_stored_msg_prefix`, because it may not be our actual
    // current message prefix.
    pub(super) fn read_msg_prefix(
        &self,
        _server_id: ServerId,
    ) -> Result<RwLockReadGuard<OwningMsgPrefix>> {
        self.msg_prefix
            .read()
            .map_err(|_| ErrorKind::LockPoisoned("stored message prefix".into()).into())
    }

    pub(super) fn read_server(&self, server_id: ServerId) -> Result<RwLockReadGuard<Server>> {
        match self.servers.get(&server_id) {
            Some(lock) => match lock.read() {
                Ok(guard) => Ok(guard),
                Err(_) => {
                    Err(ErrorKind::LockPoisoned(format!("server {:?}", server_id).into()).into())
                }
            },
            None => Err(ErrorKind::UnknownServer(server_id).into()),
        }
    }

    pub(super) fn get_server_config(&self, server_id: ServerId) -> Result<&config::Server> {
        let ServerId {
            config_idx: ServerConfigIndex(idx),
            ..
        } = server_id;
        self.config
            .servers
            .get::<usize>(idx.into())
            .ok_or_else(|| ErrorKind::UnknownServer(server_id).into())
    }

    /// Runs the given function, passing as argument the `irc` crate `IrcClient` corresponding to
    /// the given `ServerId`
    ///
    /// This function allows access to the [`irc` crate]'s [`IrcClient`] structures that represent
    /// the IRC connections that the bot is maintaining. If the [`IrcClient`] corresponding to the
    /// given `ServerId` is found successfully, the given function is run and its result is
    /// returned. Otherwise, an error is returned.
    ///
    /// For this function to be in the public API, the Cargo feature `aatxe-irc` must be enabled.
    ///
    /// [`IrcClient`]: <https://docs.rs/irc/*/irc/client/struct.IrcClient.html>
    /// [`irc` crate]: <https://docs.rs/irc>
    #[cfg(feature = "aatxe-irc")]
    pub fn with_aatxe_client<F, T>(&self, server_id: ServerId, f: F) -> Result<T>
    where
        F: FnOnce(&aatxe::IrcClient) -> Result<T>,
    {
        self.with_aatxe_client_private(server_id, f)
    }

    // TODO: Use a macro to define this function only once.
    #[cfg(not(feature = "aatxe-irc"))]
    pub(crate) fn with_aatxe_client<F, T>(&self, server_id: ServerId, f: F) -> Result<T>
    where
        F: FnOnce(&aatxe::IrcClient) -> Result<T>,
    {
        self.with_aatxe_client_private(server_id, f)
    }

    fn with_aatxe_client_private<F, T>(&self, server_id: ServerId, f: F) -> Result<T>
    where
        F: FnOnce(&aatxe::IrcClient) -> Result<T>,
    {
        f(self
            .aatxe_clients
            .read()
            .map_err(|_poisoned_guard| {
                ErrorKind::LockPoisoned("the server connections (`aatxe_clients`)".into())
            })?
            .get(&server_id)
            .ok_or(ErrorKind::UnknownServer(server_id))?)
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
        match self.read_server(server_id) {
            Ok(s) => s.socket_addr_string.clone(),
            Err(e) => format!("<unknown server {:?} ({})>", server_id, e),
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
