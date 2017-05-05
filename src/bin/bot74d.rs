extern crate clap;
extern crate env_logger;
extern crate irc_bot;

#[macro_use]
extern crate log;

use irc_bot::modules;

const PROGRAM_NAME: &'static str = "bot74d";

fn main() {
    let args = clap::App::new(PROGRAM_NAME)
        .arg(clap::Arg::with_name("config-file")
                 .short("c")
                 .default_value("config.json"))
        .get_matches();

    env_logger::init().expect("error: failed to initialize logging");

    irc_bot::run(args.value_of("config-file").expect("default missing?"),
                 |err| {
                     error!("{}", err);
                     irc_bot::ErrorReaction::Proceed
                 },
                 &[modules::default(), modules::test()]);
}
