use pircolate;
use std::error;
use std::str;
use std::string;

error_chain! {
    foreign_links {
        Utf8Error(str::Utf8Error);
        FromUtf8Error(string::FromUtf8Error);
    }

    links {
        Pircolate(pircolate::error::Error, pircolate::error::ErrorKind);
    }

    errors {
        Other(inner: Box<error::Error + Send>) {
            description("there was an unspecified problem with an IRC message")
            display("{}", inner)
        }
    }
}
