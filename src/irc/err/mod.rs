use mio;
use pircolate;
use std::io;

error_chain! {
    foreign_links {
        Io(io::Error);
    }

    links {
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
    }
}
