pub use self::err::Error;
pub use self::err::ErrorKind;
pub use self::err::Result;
use pircolate;

pub mod connection;
pub mod client;

mod err;

pub type Message = pircolate::Message;
