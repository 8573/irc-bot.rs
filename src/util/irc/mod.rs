use inlinable_string::InlinableString;
use serde;
use serde::Deserialize;
use serde::Deserializer;
use smallvec::SmallVec;
use std::cmp::Ordering;
use std::fmt;
use std::ops::Deref;
use std::result::Result as StdResult;
use std::str::FromStr;
use string_cache::DefaultAtom;
use util::regex::Regex;
use util::STATIC_REGEX_PARSE_ERR_MSG;

error_chain! {
    errors {
        InvalidChannelName(input: DefaultAtom) {
            description("encountered invalid IRC channel name"),
            display("Invalid IRC channel name: {:?}", input.as_ref()),
        }
    }
}

lazy_static! {
    /// This is a [`lazy_static`] item containing a non-anchored regex that matches an IRC channel
    /// name as specified in [IETF RFC 2812, section 2.3.1], accounting for [erratum 3783], under
    /// the most permissive interpretation of this standard (i.e., accepting such unusual channel
    /// names as `#`, with nothing after the leading sigil).
    ///
    /// For the avoidance of controversy, we note that, as of the end of November 2018, [IRCv3]
    /// does not appear to have published any conflicting standard.
    ///
    /// [IETF RFC 2812, section 2.3.1]: <https://tools.ietf.org/html/rfc2812#section-2.3.1>
    /// [IRCv3]: <https://ircv3.net>
    /// [`lazy_static`]: <https://docs.rs/lazy_static/*/lazy_static/>
    /// [erratum 3783]: <https://www.rfc-editor.org/errata/eid3783>
    pub static ref CHANNEL_NAME_REGEX: Regex = Regex::from_str(
        r"(?:[#&+]|![[:upper:][:digit:]]{5})[^\x00\a\r\n ,:]{0,49}(?::[^\x00\a\r\n ,:]{0,49})?"
    ).expect(STATIC_REGEX_PARSE_ERR_MSG);
}

/// Compares two strings case-insensitively, using the IRC rules for case-folding.
///
/// This function optimizes for comparing short strings such as nicknames and channel names.
pub fn case_insensitive_str_cmp<S1, S2>(x: S1, y: S2) -> Ordering
where
    S1: Into<InlinableString>,
    S2: Into<InlinableString>,
{
    type Buffer = SmallVec<[u8; 64]>;

    let mut x = x.into();
    let mut y = y.into();

    x.make_ascii_lowercase();
    y.make_ascii_lowercase();

    let mut x = Buffer::from(x.as_bytes());
    let mut y = Buffer::from(y.as_bytes());

    fn finish_irc_lowercasing(s: &mut Buffer) {
        for mut c in s {
            *c = match c {
                b'[' => b'{',
                b']' => b'}',
                b'\\' => b'|',
                b'~' => b'^',
                _ => continue,
            }
        }
    }

    finish_irc_lowercasing(&mut x);
    finish_irc_lowercasing(&mut y);

    x.cmp(&y)
}

/// A string type representing the name of an IRC channel.
///
/// This wrapper around an interned string (specifically, a Servo [`Atom`]) ensures that the string
/// is a valid IRC channel name and implements comparison operations as appropriate for IRC channel
/// names, comparing them case-insensitively per IRC's particular rules for such comparisons.
///
/// [`Atom`]: <https://docs.rs/string_cache/*/string_cache/atom/struct.Atom.html>
#[derive(Clone, Debug)]
pub struct ChannelName(DefaultAtom);

impl ChannelName {
    /// Constructs a new `ChannelName` from a string, verifying that the whole string is a single
    /// match of [`CHANNEL_NAME_REGEX`].
    ///
    /// An `Err` will be returned if [`CHANNEL_NAME_REGEX`] does not match against the whole given
    /// string.
    ///
    /// [`CHANNEL_NAME_REGEX`]: <struct.CHANNEL_NAME_REGEX.html>
    pub fn new<S>(name: S) -> Result<Self>
    where
        S: Into<DefaultAtom>,
    {
        let name = name.into();

        if CHANNEL_NAME_REGEX.is_match(&name) {
            Ok(ChannelName(name))
        } else {
            Err(ErrorKind::InvalidChannelName(name).into())
        }
    }
}

impl Deref for ChannelName {
    type Target = DefaultAtom;

    fn deref(&self) -> &Self::Target {
        let ChannelName(inner) = self;
        inner
    }
}

impl Ord for ChannelName {
    fn cmp(&self, other: &Self) -> Ordering {
        case_insensitive_str_cmp(self.as_ref(), other.as_ref())
    }
}

impl PartialOrd for ChannelName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ChannelName {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ChannelName {}

impl<'de> Deserialize<'de> for ChannelName {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(ChannelNameDeserializationVisitor)
    }
}

struct ChannelNameDeserializationVisitor;

impl<'de> serde::de::Visitor<'de> for ChannelNameDeserializationVisitor {
    type Value = ChannelName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a string that can be parsed as an IRC channel name"
        )
    }

    fn visit_str<E>(self, input: &str) -> StdResult<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Self::Value::new(input).map_err(serde::de::Error::custom)
    }

    fn visit_string<E>(self, input: String) -> StdResult<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Self::Value::new(input).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs `ChannelName`s from strings by using the data constructor directly, bypassing
    /// the check that the strings are valid channel names.
    ///
    /// It normally would be bad to bypass such a check, but it's good as used below, because it
    /// means that we test `case_insensitive_str_cmp` over a broader set of strings.
    fn unchecked_channel_names(
        a: String,
        b: String,
        c: String,
    ) -> (ChannelName, ChannelName, ChannelName) {
        (
            ChannelName(a.into()),
            ChannelName(b.into()),
            ChannelName(c.into()),
        )
    }

    // Note that "!p || q" should be read as "p implies q".

    // To run rustfmt on this code, temporarily change the `quickcheck! {...}` to `mod qc {...}`.
    // Beware, however, of rustfmt's adding trailing commas, which `quickcheck!` doesn't accept.
    quickcheck! {
        fn casefold_transitive_lt(a: String, b: String, c: String) -> bool {
            let (a, b, c) = unchecked_channel_names(a, b, c);

            !(a < b && b < c) || a < c
        }

        fn casefold_transitive_eq(a: String, b: String, c: String) -> bool {
            let (a, b, c) = unchecked_channel_names(a, b, c);

            !(a == b && b == c) || a == c
        }

        fn casefold_transitive_gt(a: String, b: String, c: String) -> bool {
            let (a, b, c) = unchecked_channel_names(a, b, c);

            !(a > b && b > c) || a > c
        }
    }
}
