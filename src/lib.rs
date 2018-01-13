#![recursion_limit="128"]

// TODO: Determine why `error-chain` triggers the `unused_doc_comment` warning.
#![allow(unused_doc_comment)]

extern crate clap;
extern crate crossbeam_channel;
extern crate crossbeam_utils;
extern crate irc;
extern crate itertools;
extern crate rand;
extern crate rando;
extern crate regex;
extern crate serde;
extern crate serde_yaml;
extern crate smallvec;
extern crate uuid;
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
