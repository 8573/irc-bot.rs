extern crate clap;
extern crate crossbeam;
extern crate irc;
extern crate itertools;
extern crate parking_lot;
extern crate skimmer;
extern crate uuid;
extern crate yaml_rust;

#[macro_use]
extern crate bitflags;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate yamlette;

pub use self::core::Config;
pub use self::core::ErrorReaction;
pub use self::core::run;

pub mod core;
pub mod modules;
pub mod util;
