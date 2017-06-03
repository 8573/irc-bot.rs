extern crate clap;
extern crate crossbeam;
extern crate futures;
extern crate itertools;
extern crate parking_lot;
extern crate pircolate;
extern crate skimmer;
extern crate tokio_core;
extern crate tokio_irc_client;
extern crate uuid;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;

#[macro_use]
extern crate yamlette;

pub use self::core::Config;
pub use self::core::ErrorReaction;
pub use self::core::run;

pub mod core;
pub mod modules;
