use super::BotCommand;
use super::Error;
use super::ErrorKind;
use super::MsgPrefix;
use super::Result;
use super::Server;
use super::ServerId;
use super::State;
use super::config;
use super::irc_msgs::OwningMsgPrefix;
use rand::StdRng;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::MutexGuard;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;
use util::lock::RwLockExt;

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

    pub fn have_admin(&self,
MsgPrefix { nick: nick_1, user: user_1, host: host_1 }: MsgPrefix) -> Result<bool>{
        Ok(self.config.admins.iter().any(|&config::Admin {
             nick: ref nick_2,
             user: ref user_2,
             host: ref host_2,
         }| {
            check_admin_cred(nick_1, nick_2) && check_admin_cred(user_1, user_2) &&
                check_admin_cred(host_1, host_2)
        }))
    }

    pub(super) fn read_msg_prefix<'a>(
        &'a self,
        server_id: ServerId,
    ) -> Result<RwLockReadGuard<'a, OwningMsgPrefix>> {
        self.read_msg_prefixes()?
            .get(&server_id)
            .ok_or(Error::from(ErrorKind::UnknownServer(server_id)))?
            .read_clean("a stored message prefix")
    }

    pub(super) fn read_msg_prefixes<'a>(
        &'a self,
    ) -> Result<RwLockReadGuard<'a, BTreeMap<ServerId, RwLock<OwningMsgPrefix>>>> {
        self.msg_prefixes.read_clean(
            "the associative array of the bot's per-server message prefixes",
        )
    }

    pub(super) fn read_server<'a>(
        &'a self,
        server_id: ServerId,
    ) -> Result<RwLockReadGuard<'a, Server>> {
        match self.read_servers()?.get(&server_id) {
            Some(lock) => {
                match lock.read() {
                    Ok(guard) => Ok(guard),
                    Err(_) => Err(
                        ErrorKind::LockPoisoned(
                            format!("server {}", server_id.uuid.hyphenated()).into(),
                        ).into(),
                    ),
                }
            }
            None => Err(ErrorKind::UnknownServer(server_id).into()),
        }
    }

    pub(super) fn read_servers<'a>(
        &'a self,
    ) -> Result<RwLockReadGuard<'a, BTreeMap<ServerId, RwLock<Server>>>> {
        self.servers.read_clean("the server list")
    }

    pub(super) fn register_server(&self, server: Server) -> Result<()> {
        let server_id = server.id;
        let msg_prefix = RwLock::new(OwningMsgPrefix::from_string(format!(
            "{}!{}@",
            self.config.nickname,
            self.config.username
        )));

        let (mut servers, mut msg_prefixes) = self.server_write_locks()?;

        if servers.contains_key(&server_id) {
            return Err(ErrorKind::ServerRegistryClash(server_id).into());
        }

        servers.insert(server_id, RwLock::new(server));
        msg_prefixes.insert(server_id, msg_prefix);

        Ok(())
    }

    /// Clears information about the server with the given ID out of the `State`.
    ///
    /// It is not considered an error if the given ID is invalid.
    pub(super) fn deregister_server(&self, server_id: ServerId) -> Result<()> {
        let (mut servers, mut msg_prefixes) = self.server_write_locks()?;

        servers.remove(&server_id);
        msg_prefixes.remove(&server_id);

        Ok(())
    }

    /// Acquires all the write locks for the per-server associative arrays at once, so that no-one
    /// sees them while they're only partially updated.
    fn server_write_locks(
        &self,
    ) -> Result<
        (RwLockWriteGuard<BTreeMap<ServerId, RwLock<Server>>>,
         RwLockWriteGuard<BTreeMap<ServerId, RwLock<OwningMsgPrefix>>>),
    > {
        Ok((
            self.servers.write_clean("the server list")?,
            self.msg_prefixes.write_clean(
                "the associative array of the bot's \
                 per-server message prefixes",
            )?,
        ))
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
            Ok(s) => s.socket_addr_string.clone(),
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
