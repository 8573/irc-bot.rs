#![recursion_limit = "128"]
// TODO: Determine why `error-chain` triggers the `unused_doc_comment` warning.
#![allow(unused_doc_comment)]
#![deny(unsafe_code)]

extern crate clockpro_cache;
extern crate crossbeam_channel;
extern crate irc;
extern crate itertools;
extern crate quantiles;
extern crate rand;
extern crate rando;
extern crate ref_slice;
extern crate regex;
extern crate serde_yaml;
extern crate smallbitvec;
extern crate smallvec;
extern crate string_cache;
extern crate strum;
extern crate try_map;
extern crate url;
extern crate url_serde;
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

#[macro_use]
extern crate strum_macros;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

pub use self::core::*;

pub mod modules;
pub mod util;

mod core;
