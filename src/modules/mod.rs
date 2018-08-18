pub use self::default::mk as default;
pub use self::test::mk as test;
use core::Module;

mod default;
mod test;

/// A list of all bot modules provided by this library, suitable for passing to [`run`].
///
/// [`run`]: <../fn.run.html>
pub const ALL: &[fn() -> Module] = &[default, test];
