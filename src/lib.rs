extern crate clap;
extern crate irc;
extern crate itertools;
extern crate uuid;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;

pub use self::core::ErrorReaction;
pub use self::core::run;

pub mod core;
pub mod modules;
