use super::BotCmdResult;
use super::ErrorKind;
use super::HandlerContext;
use super::Module;
use super::ModuleFeatureRef;
use super::MsgMetadata;
use super::Result;
use super::State;
use super::TriggerHandler;
use rando::Rando;
use regex::Regex;
use std::borrow::Cow;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use util;
use uuid::Uuid;

#[derive(CustomDebug)]
pub struct Trigger {
    pub name: Cow<'static, str>,

    pub provider: Arc<Module>,

    pub regex: Arc<RwLock<Regex>>,

    pub priority: TriggerPriority,

    #[debug(skip)]
    pub(super) handler: Arc<TriggerHandler>,

    pub help_msg: Cow<'static, str>,

    pub uuid: Uuid,
}

pub(super) struct TemporaryTrigger {
    pub(super) inner: Trigger,
    pub(super) activation_limit: u16,
}

#[derive(Debug)]
pub enum TriggerAttr {
    /// Use this attribute for triggers that should trigger even on messages that aren't addressed
    /// to the bot.
    ///
    /// As of 2018-01-11, this doesn't actually do anything yet.
    AlwaysWatching,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum TriggerPriority {
    /// Designates the trigger as having minimum priority.
    Minimum,

    /// Designates the trigger as having low priority. This is appropriate for triggers that are
    /// intended primarily for jocular, fun, playful, comedic, humorous, levitous, frivolous, or
    /// otherwise non-serious purposes.
    Low,

    /// Designates the trigger as having medium priority.
    Medium,

    /// Designates the trigger as having high priority. This is appropriate for triggers that
    /// implement important functionality of a particular bot.
    High,

    /// Designates the trigger as having maximum priority.
    Maximum,
}

impl Trigger {
    fn read_regex(&self) -> Result<RwLockReadGuard<Regex>> {
        self.regex.read().map_err(|_| {
            ErrorKind::LockPoisoned(
                format!(
                    "the regex for trigger {uuid} ({name:?})",
                    name = self.name,
                    uuid = self.uuid.hyphenated()
                ).into(),
            ).into()
        })
    }
}

/// Returns `None` if no trigger matched.
pub(super) fn run_any_matching(
    state: &State,
    text: &str,
    msg_metadata: &MsgMetadata,
) -> Result<Option<BotCmdResult>> {
    let mut trigger = None;

    for (_priority, triggers) in state.triggers.iter().rev() {
        if triggers.is_empty() {
            continue;
        }

        if let Some(t) = triggers
            .rand_iter()
            .with_rng(state.rng()?.deref_mut())
            .filter(|t| t.read_regex().map(|rx| rx.is_match(text)).unwrap_or(false))
            .next()
        {
            trigger = Some(t);
            break;
        }
    }

    let trigger = match trigger {
        Some(t) => t,
        None => return Ok(None),
    };

    let ctx = HandlerContext {
        state,
        this_feature: ModuleFeatureRef::Trigger(trigger),
        request_origin: msg_metadata.dest,
        invoker: msg_metadata.prefix,
        __nonexhaustive: (),
    };

    let args = trigger.read_regex()?.captures(text).expect(
        "We shouldn't have reached this point if the \
         trigger didn't match!",
    );

    Ok(Some(util::run_handler(
        "trigger",
        trigger.name.clone(),
        || trigger.handler.run(ctx, args),
    )?))
}
