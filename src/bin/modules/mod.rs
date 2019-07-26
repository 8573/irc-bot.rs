pub use self::default::mk as default;
pub use self::quote::mk as quote;
pub use self::test::mk as test;
use irc_bot::Module;

mod default;
mod quote;
mod test;

/// A list of all bot modules provided by this library, suitable for passing to [`run`].
///
/// [`run`]: <../fn.run.html>
pub const ALL: &[fn() -> Module] = &[default, quote, test];
