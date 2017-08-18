extern crate clap;
extern crate itertools;
extern crate mio;
extern crate parking_lot;
extern crate pircolate;
extern crate rustls;
extern crate skimmer;
extern crate uuid;
extern crate webpki_roots;
extern crate yak_irc;

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
