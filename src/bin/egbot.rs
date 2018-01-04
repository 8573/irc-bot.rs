extern crate clap;
extern crate env_logger;
extern crate irc_bot;

#[macro_use]
extern crate log;

use irc_bot::modules;

fn main() {
    let args = clap::App::new("egbot")
        .arg(
            clap::Arg::with_name("config-file")
                .short("c")
                .default_value("config.yaml"),
        )
        .get_matches();

    env_logger::init().expect("error: failed to initialize logging");

    irc_bot::run(
        irc_bot::Config::try_from_path(args.value_of("config-file").expect("default missing?")),
        |err| {
            error!("{:?}", err);
            irc_bot::ErrorReaction::Proceed
        },
        &[modules::default, modules::test],
    );
}
