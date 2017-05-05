extern crate clap;
extern crate irc;
extern crate itertools;
extern crate uuid;

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate log;

use std::io;
use std::io::Write as IoWrite;

mod core;
mod modules;

const PROGRAM_NAME: &'static str = "bot74d";

fn main() {
    let args = clap::App::new(PROGRAM_NAME)
        .arg(clap::Arg::with_name("config-file")
                 .short("c")
                 .default_value("config.json"))
        .get_matches();

    let log_lvl = log::LogLevelFilter::Info;

    log::set_logger(|max_log_lvl| {
                        max_log_lvl.set(log_lvl);
                        Box::new(LogBackend { log_lvl: log_lvl })
                    })
            .expect("error: failed to initialize logging");

    core::run(args.value_of("config-file").expect("default missing?"),
              |err| {
                  error!("{}", err);
                  core::ErrorReaction::Proceed
              },
              &[modules::default(), modules::test()]);
}


struct LogBackend {
    log_lvl: log::LogLevelFilter,
}

impl log::Log for LogBackend {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= self.log_lvl
    }

    fn log(&self, record: &log::LogRecord) {
        if !self.enabled(record.metadata()) {
            return;
        }
        writeln!(io::stderr(), "{}: {}", record.level(), record.args()).expect("stderr broken?");
    }
}
