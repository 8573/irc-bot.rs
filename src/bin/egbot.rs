#![forbid(unsafe_code)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate strum_macros;

extern crate strum;

extern crate env_logger;
extern crate irc_bot;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate quickcheck;


mod modules;

fn main() {
    let args = clap::App::new("egbot")
        .arg(
            clap::Arg::with_name("config-file")
                .long("config-file")
                .short("c")
                .default_value("config.yaml"),
        )
        .arg(
            clap::Arg::with_name("data-dir")
                .long("data-dir")
                .short("d")
                .default_value("data"),
        )
        .arg(
            clap::Arg::with_name("error-verbosity")
                .long("error-verbosity")
                .possible_values(&ErrorVerbosity::variants())
                .case_insensitive(true)
                .default_value("Display"),
        )
        .get_matches();

    env_logger::init();

    let error_verbosity =
        value_t!(args, "error-verbosity", ErrorVerbosity).unwrap_or_else(|err| err.exit());

    irc_bot::run(
        irc_bot::Config::try_from_path(args.value_of("config-file").expect("default missing?")),
        args.value_of("data-dir").expect("default missing?"),
        move |err| {
            match error_verbosity {
                ErrorVerbosity::Display => error!("{}", err),
                ErrorVerbosity::Debug => error!("{:?}", err),
            }
            irc_bot::ErrorReaction::Proceed
        },
        modules::ALL,
    );
}

arg_enum! {
    #[derive(Debug)]
    enum ErrorVerbosity {
        Display,
        Debug
    }
}
