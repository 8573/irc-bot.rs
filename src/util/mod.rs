use core::ErrorKind;
use core::Result;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::panic;

pub(crate) mod fmt;
pub(crate) mod lock;
pub mod regex;
pub mod yaml;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[must_use]
pub(crate) struct MustUse<T>(pub T);

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

/// Calls `ToOwned::to_owned` on the argument and wraps the result in `Cow::Owned`.
pub fn to_cow_owned<T>(x: &T) -> Cow<'static, T>
where
    T: ToOwned + ?Sized,
{
    Cow::Owned(x.to_owned())
}

#[derive(Clone, Debug)]
pub(crate) struct Munge<'a> {
    string: &'a str,
    munge_points: SmallVec<[usize; 32]>,
    outgoing_str: Option<&'a str>,
    pos: usize,
    sep: &'a str,
    munging: bool,
}

/// Returns an iterator over string slices whose concatenation equals the given `string`, except
/// with zero-width spaces inserted into each multi-`char` occurrence of any of the given
/// `needles`.
///
/// Needles that are a single `char` long are ignored.
///
/// TODO: Split a generalized version of this out as a new crate.
///
/// TODO: See the logs of <ircs://irc.mozilla.org/c74d> from 2018-10-17 regarding possible munging
/// characters.
///
/// TODO: A generalized version perhaps should operate over graphemes (as does the function
/// `create_non_highlighting_name` in <https://github.com/nuxeh/url-bot-rs>) rather than Unicode
/// scalar values; I should investigate the distinction more once my oaths permit.
pub(crate) fn zwsp_munge<'a, 'b, I, S>(string: &'a str, needles: I) -> Munge<'a>
where
    I: IntoIterator<Item = S>,
    S: 'b + AsRef<str>,
{
    // TODO: Maybe increase the stack space allocated here when splitting this function out?
    let mut munge_points = SmallVec::<[usize; 32]>::new();

    for (needle, needle_first_char_byte_len) in needles.into_iter().filter_map(|needle| {
        needle.as_ref().char_indices().nth(1).map(
            |(second_char_index_in_needle, _second_char_in_needle): (usize, char)| {
                (needle, second_char_index_in_needle)
            },
        )
    }) {
        for pos in string
            .match_indices(needle.as_ref())
            .map(|(needle_index_in_string, _)| needle_index_in_string + needle_first_char_byte_len)
        {
            munge_points.push(pos);
        }
    }

    // Sort the vector in reverse order, so that the "first" points are at the end, for use as a
    // stack.
    munge_points.sort_unstable_by(|a, b| b.cmp(a));
    munge_points.dedup();

    Munge {
        string,
        munge_points,
        outgoing_str: None,
        pos: 0,
        sep: "\u{200B}",
        munging: false,
    }
}

impl<'a> Iterator for Munge<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.string.len() {
            return None;
        }

        let next_munge_point = self.munge_points.last().cloned();

        match (next_munge_point, self.munging) {
            (Some(i), false) => {
                self.munging = true;
                self.string.get(self.pos..i)
            }
            (Some(i), true) => {
                self.munge_points.pop();
                self.pos = i;
                self.munging = false;
                Some(self.sep)
            }
            (None, false) => {
                let r = self.string.get(self.pos..);
                self.pos = self.string.len();
                r
            }
            (None, true) => unreachable!(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            ExactSizeIterator::len(self),
            Some(ExactSizeIterator::len(self)),
        )
    }
}

impl<'a> ExactSizeIterator for Munge<'a> {
    fn len(&self) -> usize {
        self.munge_points.len() * 2 + if !self.string.is_empty() { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zwsp_munge_examples() {
        let no_strs: &[&'static str] = &[];

        let mut it = zwsp_munge("", no_strs);
        let it2 = it.clone();

        assert_eq!(it.len(), 0);

        assert_eq!(it.next(), None);

        assert_eq!(&it2.collect::<String>(), "");

        let mut it = zwsp_munge("", &["abc", "xyz", "quux"]);
        let it2 = it.clone();

        assert_eq!(it.len(), 0);

        assert_eq!(it.next(), None);

        assert_eq!(&it2.collect::<String>(), "");

        let mut it = zwsp_munge("abc xyz quux", no_strs);
        let it2 = it.clone();

        assert_eq!(it.len(), 1);

        assert_eq!(it.next(), Some("abc xyz quux"));
        assert_eq!(it.next(), None);

        assert_eq!(&it2.collect::<String>(), "abc xyz quux");

        let mut it = zwsp_munge("lorem ipsum", &["quux", "psu"]);
        let it2 = it.clone();

        assert_eq!(it.len(), 3);

        assert_eq!(it.next(), Some("lorem ip"));
        assert_eq!(it.next(), Some("\u{200B}"));
        assert_eq!(it.next(), Some("sum"));
        assert_eq!(it.next(), None);

        assert_eq!(&it2.collect::<String>(), "lorem ip\u{200B}sum");

        let mut it = zwsp_munge("foo bar baz", &["ba", "oo"]);
        let it2 = it.clone();

        assert_eq!(it.len(), 7);

        assert_eq!(it.next(), Some("fo"));
        assert_eq!(it.next(), Some("\u{200B}"));
        assert_eq!(it.next(), Some("o b"));
        assert_eq!(it.next(), Some("\u{200B}"));
        assert_eq!(it.next(), Some("ar b"));
        assert_eq!(it.next(), Some("\u{200B}"));
        assert_eq!(it.next(), Some("az"));
        assert_eq!(it.next(), None);

        assert_eq!(
            &it2.collect::<String>(),
            "fo\u{200B}o b\u{200B}ar b\u{200B}az"
        );
    }

    quickcheck! {
        fn zwsp_munge_exact_size(string: String, needles: Vec<String>) -> () {
            let it = zwsp_munge(&string, needles);
            let claimed_len = it.len();
            assert_eq!(claimed_len, it.count());
        }
    }
}
