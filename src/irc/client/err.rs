use irc::connection;
use pircolate;
use std::io;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    links {
        Connection(connection::Error, connection::ErrorKind);
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
    }
}
