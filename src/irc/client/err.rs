use irc::connection;
use irc::message;
use pircolate;
use std::io;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    links {
        Message(message::Error, message::ErrorKind);
        Connection(connection::Error, connection::ErrorKind);
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
    }
}
