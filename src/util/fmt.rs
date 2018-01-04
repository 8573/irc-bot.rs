use core;
use std::any::Any;
use std::borrow::Cow;
use std::cell::Cell;
use std::error;
use std::fmt;

pub(crate) struct FmtAny<'a>(pub(crate) &'a Any);

macro_rules! impl_fmt {
    (@inner($self:ident, $formatter:ident, $trait:path, {})) => {
        write!($formatter, "<unknown: {:?}>", $self.0)
    };
    (@inner($self:ident, $formatter:ident, $trait:path, {$cur_ty:ty; $($todo_ty:ty;)*})) => {
        match $self.0.downcast_ref::<$cur_ty>() {
            Some(concrete) => <$trait>::fmt(concrete, $formatter),
            None => {
                impl_fmt!(@inner($self, $formatter, $trait, {$($todo_ty;)*}))
            }
        }
    };
    ($trait:path {$($ty:ty;)*}) => {
        impl<'a> $trait for FmtAny<'a> {
            fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                impl_fmt!(@inner(self, formatter, $trait, {$($ty;)*}))
            }
        }
    };
}

impl_fmt!(fmt::Display {
    &str;
    String;
    Cow<'static, str>;
    core::Error;
});

impl_fmt!(fmt::Debug {
    &str;
    String;
    Cow<'static, str>;
    core::Error;
});
