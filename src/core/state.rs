use super::BotCommand;
use super::ErrorKind;
use super::ModuleFeatureKind;
use super::MsgPrefix;
use super::Result;
use super::ServerId;
use super::State;
use super::config;
use std::borrow::Cow;

impl State {
    pub fn nick(&self) -> Result<String> {
        self.msg_prefix
            .read()
            .parse()
            .nick
            .ok_or(ErrorKind::NicknameUnknown.into())
            .map(ToOwned::to_owned)
    }

    pub fn command(&self, name: &str) -> Result<Option<&BotCommand>> {
        Ok(self.commands.get(name))
    }

    pub fn command_names(&self) -> Result<Vec<Cow<'static, str>>> {
        Ok(self.commands.keys().cloned().collect())
    }

    pub fn have_module_feature(&self, kind: ModuleFeatureKind, name: &str) -> Result<bool> {
        match kind {
            ModuleFeatureKind::Command => Ok(self.commands.contains_key(name)),
            ModuleFeatureKind::Trigger => unimplemented!(),
        }
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

    /// TODO: This should return something less allocate-y.
    pub(super) fn server_socket_addr_string(&self, server_id: ServerId) -> String {
        self.servers
            .get(&server_id)
            .map(|s| s.read().socket_addr_string.clone())
            .unwrap_or_else(|| {
                format!("<unknown server {}>", server_id.uuid.hyphenated())
            })
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
