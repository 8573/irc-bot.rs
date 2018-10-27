use super::BotCmdResult;
use super::BotCommand;
use super::Error;
use super::ErrorReaction;
use super::MsgDest;
use super::MsgMetadata;
use super::MsgPrefix;
use super::Result;
use super::State;
use super::Trigger;
use regex::Captures;
use std::panic::RefUnwindSafe;
use std::panic::UnwindSafe;
use yaml_rust::Yaml;

pub trait ErrorHandler: Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
    /// Handles an error.
    ///
    /// The handler is given ownership of the error so that the handler can easily store the error
    /// somewhere if desired.
    fn run(&self, Error) -> ErrorReaction;
}

impl<T> ErrorHandler for T
where
    T: Fn(Error) -> ErrorReaction + Send + Sync + UnwindSafe + RefUnwindSafe + 'static,
{
    fn run(&self, err: Error) -> ErrorReaction {
        self(err)
    }
}

pub trait BotCmdHandler: Send + Sync + UnwindSafe + RefUnwindSafe {
    fn run(&self, HandlerContext, &Yaml) -> BotCmdResult;
}

impl<F, R> BotCmdHandler for F
where
    F: Fn(HandlerContext, &Yaml) -> R + Send + Sync + UnwindSafe + RefUnwindSafe,
    R: Into<BotCmdResult>,
{
    fn run(&self, ctx: HandlerContext, arg: &Yaml) -> BotCmdResult {
        self(ctx, arg).into()
    }
}

pub trait TriggerHandler: Send + Sync + UnwindSafe + RefUnwindSafe {
    fn run(&self, HandlerContext, Captures) -> BotCmdResult;
}

impl<F, R> TriggerHandler for F
where
    F: Fn(HandlerContext, Captures) -> R + Send + Sync + UnwindSafe + RefUnwindSafe,
    R: Into<BotCmdResult>,
{
    fn run(&self, ctx: HandlerContext, args: Captures) -> BotCmdResult {
        self(ctx, args).into()
    }
}

pub trait ModuleLoadHandler: Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
    fn run(&self, &State) -> Result<()>;
}

impl<F, R> ModuleLoadHandler for F
where
    F: Fn(&State) -> R + Send + Sync + UnwindSafe + RefUnwindSafe + 'static,
    R: Into<Result<()>>,
{
    fn run(&self, state: &State) -> Result<()> {
        self(state).into()
    }
}

#[derive(CustomDebug)]
pub struct HandlerContext<'s, 'm> {
    /// The bot state
    pub state: &'s State,

    /// The module feature for which this handler is running
    pub this_feature: ModuleFeatureRef<'s>,

    /// This field identifies the channel or other notional location in which originated the
    /// request that caused this handler to be run.
    pub request_origin: MsgDest<'m>,

    /// This field identifies the user (or fellow bot) who caused this handler to be run.
    pub invoker: MsgPrefix<'m>,

    #[debug(skip)]
    #[doc(hidden)]
    pub(super) __nonexhaustive: (),
}

#[derive(Debug)]
pub enum ModuleFeatureRef<'s> {
    Command(&'s BotCommand),
    Trigger(&'s Trigger),
}

impl<'s, 'm> HandlerContext<'s, 'm> {
    /// Returns the `MsgMetadata` for the message that caused this handler to be run.
    pub fn request_metadata(&self) -> MsgMetadata<'m> {
        MsgMetadata {
            dest: self.request_origin,
            prefix: self.invoker,
        }
    }

    /// Returns a guess at the destination to which any message returned by this handler will be
    /// sent.
    ///
    /// `ctx.guess_reply_dest()` is equivalent to
    /// `ctx.state.guess_reply_dest(&ctx.request_metadata())`.
    pub fn guess_reply_dest(&self) -> Result<MsgDest<'m>> {
        self.state.guess_reply_dest(&self.request_metadata())
    }

    // TODO
    // pub fn module_data(&self) -> Result<...> {
    //     let module_id = &self.this_feature.provider().(...);
    //     self.state.(...)
    // }
}
