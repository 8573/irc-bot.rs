use core::ErrorKind;
use core::Result;
use std::borrow::Cow;
use std::panic;

pub(crate) mod fmt;
pub mod yaml;

pub(crate) fn run_handler<S1, S2, F, R>(
    feature_kind: S1,
    feature_name: S2,
    handler_invocation: F,
) -> Result<R>
where
    S1: Into<Cow<'static, str>>,
    S2: Into<Cow<'static, str>>,
    F: FnOnce() -> R + panic::UnwindSafe,
{
    panic::catch_unwind(handler_invocation).map_err(|panic_payload| {
        ErrorKind::HandlerPanic(feature_kind.into(), feature_name.into(), panic_payload).into()
    })
}
