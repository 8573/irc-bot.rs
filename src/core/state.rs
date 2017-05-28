use super::BotCommand;
use super::ErrorKind;
use super::ModuleFeatureKind;
use super::MsgPrefix;
use super::Result;
use super::State;
use irc::client::prelude::*;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::hash::Hash;

impl<'server, 'modl> State<'server, 'modl> {
    pub fn nick(&self) -> &str {
        self.server.current_nickname()
    }

    pub fn command(&self, name: &str) -> Option<&BotCommand> {
        self.commands.get(name)
    }

    pub fn command_names(&self) -> Vec<Cow<'static, str>> {
        self.commands.keys().cloned().collect()
    }

    pub fn have_module_feature(&self, kind: ModuleFeatureKind, name: &str) -> bool {
        match kind {
            ModuleFeatureKind::Command => self.commands.contains_key(name),
            ModuleFeatureKind::Trigger => unimplemented!(),
        }
    }

    pub fn have_owner(&self, MsgPrefix { nick, user, .. }: MsgPrefix) -> Result<bool> {
        let cfg_key = "owner-auth-check-policy".to_string();
        let default = "nick-only".to_string();
        let policy = self.query_cfg(&cfg_key).unwrap_or(&default);
        let (match_nick, match_user) = match () {
            () if policy == "nick+user" => (true, true),
            () if policy == "nick-only" => (true, false),
            () if policy == "user-only" => (false, true),
            _ => {
                bail!(ErrorKind::Config(cfg_key,
                                        "is not `nick+user`, `nick-only`, or `user-only`".into()))
            }
        };

        Ok(match self.server.config().owners {
               Some(ref vec) => {
                   vec.iter()
                       .map(String::as_ref)
                       .map(Some)
                       .any(|owner| {
                                (!match_nick || owner == nick) && (!match_user || owner == user)
                            })
               }
               None => false,
           })
    }

    pub fn query_cfg<Q>(&self, key: &Q) -> Option<&String>
        where String: Borrow<Q>,
              Q: Eq + Hash
    {
        if let Some(ref options) = self.server.config().options {
            options.get(key)
        } else {
            None
        }
    }
}
