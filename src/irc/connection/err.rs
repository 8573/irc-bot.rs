use irc::message;
use pircolate;
use std::io;
use std::str;

error_chain! {
    foreign_links {
        Io(io::Error);
        Utf8Error(str::Utf8Error);
    }

    links {
        Message(message::Error, message::ErrorKind);
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
    }
}
