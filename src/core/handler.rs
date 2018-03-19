use super::BotCmdResult;
use super::Error;
use super::ErrorReaction;
use super::MsgMetadata;
use super::State;
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
    fn run(&self, &State, &MsgMetadata, &Yaml) -> BotCmdResult;
}

impl<F, R> BotCmdHandler for F
where
    F: Fn(&State, &MsgMetadata, &Yaml) -> R + Send + Sync + UnwindSafe + RefUnwindSafe,
    R: Into<BotCmdResult>,
{
    fn run(&self, state: &State, msg_md: &MsgMetadata, arg: &Yaml) -> BotCmdResult {
        self(state, msg_md, arg).into()
    }
}

pub trait TriggerHandler: Send + Sync + UnwindSafe + RefUnwindSafe {
    fn run(&self, &State, &MsgMetadata, Captures) -> BotCmdResult;
}

impl<F, R> TriggerHandler for F
where
    F: Fn(&State, &MsgMetadata, Captures) -> R + Send + Sync + UnwindSafe + RefUnwindSafe,
    R: Into<BotCmdResult>,
{
    fn run(&self, state: &State, msg_md: &MsgMetadata, args: Captures) -> BotCmdResult {
        self(state, msg_md, args).into()
    }
}
