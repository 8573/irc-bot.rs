#![recursion_limit = "128"]
// TODO: Determine why `error-chain` triggers the `unused_doc_comment` warning.
#![allow(unused_doc_comment)]
#![deny(unsafe_code)]

extern crate crossbeam_channel;
extern crate irc;
extern crate itertools;
extern crate rand;
extern crate rando;
extern crate regex;
extern crate serde_yaml;
extern crate smallvec;
extern crate try_map;
extern crate uuid;
extern crate walkdir;
extern crate yaml_rust;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate serde_derive;

pub use self::core::*;

pub mod modules;
pub mod util;

mod core;
