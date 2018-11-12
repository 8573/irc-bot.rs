use inlinable_string::InlinableString;
use inlinable_string::StringExt as InlinableStringExt;
use regex;
use regex::RegexBuilder;
use serde;
use serde::Deserialize;
use serde::Deserializer;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::result::Result as StdResult;
use std::str::FromStr;

const REGEX_SIZE_LIMIT: usize = 1 << 17;

const REGEX_ANCHOR_START: &str = r"\A(?:";

const REGEX_ANCHOR_END: &str = r")\z";

type RegexBuildResult = StdResult<regex::Regex, regex::Error>;

/// Parses a `&str` into a case-insensitive `Regex`.
fn mk_case_insensitive_regex(s: &str) -> RegexBuildResult {
    let mut rx = RegexBuilder::new(s);
    rx.case_insensitive(true);
    rx.size_limit(REGEX_SIZE_LIMIT);
    rx.dfa_size_limit(REGEX_SIZE_LIMIT);
    rx.build()
}

/// This trait is implemented for `&str` and `Regex` such that one can pass either to certain
/// functions in this library, with a `&str` being parsed into a case-insensitive `Regex`, and a
/// `Regex` being accepted with its case-sensitivity unchanged.
///
/// TODO: Merge this with our `Regex` wrapper somehow. (Maybe wait for `TryFrom`.)
pub trait IntoRegexCI {
    fn into_regex_ci(self) -> RegexBuildResult;
}

impl IntoRegexCI for regex::Regex {
    fn into_regex_ci(self) -> RegexBuildResult {
        Ok(self)
    }
}

impl<'a> IntoRegexCI for &'a str {
    fn into_regex_ci(self) -> RegexBuildResult {
        mk_case_insensitive_regex(self)
    }
}

/// A configurably [`Deserialize`]able wrapper around [`Regex`].
///
/// While there already is [`serde_regex`] for serializing and deserializing [`Regex`]es with
/// Serde, the advantage of this wrapper is that it allows for using the [`RegexBuilder`] interface
/// to configure a regex that's being deserialized. This generic type takes as a type parameter a
/// type implementing the trait [`RegexConfig`], which implementation specifies how the input
/// string that is to be parsed as a regex should be processed.
///
/// # Examples
///
/// One could use this mechanism to make certain deserialized regexes be case-insensitive (don't
/// bother implementing this, though — [this library does so already][`CaseInsensitive`]):
///
/// ```rust
/// # extern crate irc_bot;
/// # extern crate regex;
/// # extern crate serde_yaml;
/// # #[macro_use] extern crate serde_derive;
/// # use irc_bot::util::regex::Regex;
/// # use irc_bot::util::regex::RegexConfig;
/// # use serde_yaml::Result;
/// # fn main() -> Result<()> {
/// use regex::RegexBuilder;
///
/// struct CaseInsensitive;
///
/// impl RegexConfig for CaseInsensitive {
///     fn builder_from_str(input: &str) -> RegexBuilder {
///         let mut rxb = RegexBuilder::new(input);
///         rxb.case_insensitive(true);
///         rxb
///     }
/// }
///
/// #[derive(Deserialize)]
/// struct StructWithRegex {
///     n: u64,
///     regex: Regex<CaseInsensitive>,
/// }
///
/// let deserialized: StructWithRegex = serde_yaml::from_str(r###"
///     n: 123
///     regex: "bee"
/// "###)?;
///
/// assert!(deserialized.regex.is_match("Where have you BEEN?!"));
/// # Ok(())
/// # }
/// ```
///
/// It can be useful to be able to combine multiple implementations of `RegexConfig`, so this
/// library always uses a type parameter `Base`, as in the following example:
///
/// ```rust
/// # extern crate irc_bot;
/// # extern crate regex;
/// # extern crate serde_yaml;
/// # #[macro_use] extern crate serde_derive;
/// # use irc_bot::util::regex::Regex;
/// # use irc_bot::util::regex::RegexConfig;
/// # use irc_bot::util::regex::config;
/// # use serde_yaml::Result;
/// # fn main() -> Result<()> {
/// use regex::RegexBuilder;
/// use std::marker::PhantomData;
///
/// struct CaseInsensitive<Base = config::Standard>(PhantomData<Base>)
/// where
///     Base: RegexConfig;
///
/// impl<Base> RegexConfig for CaseInsensitive<Base>
/// where
///     Base: RegexConfig,
/// {
///     fn builder_from_str(input: &str) -> RegexBuilder {
///         let mut rxb = Base::builder_from_str(input);
///         rxb.case_insensitive(true);
///         rxb
///     }
/// }
///
/// #[derive(Deserialize)]
/// struct StructWithRegex {
///     n: u64,
///     regex: Regex<CaseInsensitive>,
/// }
///
/// let deserialized: StructWithRegex = serde_yaml::from_str(r###"
///     n: 123
///     regex: "bee"
/// "###)?;
///
/// assert!(deserialized.regex.is_match("Where have you BEEN?!"));
/// # Ok(())
/// # }
/// ```
///
/// [`CaseInsensitive`]: <config/struct.CaseInsensitive.html>
/// [`Deserialize`]: <https://docs.serde.rs/serde/trait.Deserialize.html>
/// [`RegexBuilder`]: <https://docs.rs/regex/*/regex/struct.RegexBuilder.html>
/// [`RegexConfig`]: <trait.RegexConfig.html>
/// [`Regex`]: <https://docs.rs/regex/*/regex/struct.Regex.html>
/// [`serde_regex`]: <https://docs.rs/serde_regex/*/serde_regex/>
#[derive(Debug)]
pub struct Regex<Cfg = config::Standard>(regex::Regex, PhantomData<Cfg>)
where
    Cfg: RegexConfig;

impl<Cfg> Regex<Cfg>
where
    Cfg: RegexConfig,
{
    pub fn into_inner(self) -> regex::Regex {
        let Regex(inner, PhantomData) = self;
        inner
    }

    fn try_from_str(input: &str) -> StdResult<Self, regex::Error> {
        Self::try_from_builder(Cfg::builder_from_str(input))
    }

    fn try_from_string(input: String) -> StdResult<Self, regex::Error> {
        Self::try_from_builder(Cfg::builder_from_string(input))
    }

    fn try_from_builder(builder: RegexBuilder) -> StdResult<Self, regex::Error> {
        builder.build().map(|rx| Regex(rx, PhantomData))
    }
}

impl<Cfg> Deref for Regex<Cfg>
where
    Cfg: RegexConfig,
{
    type Target = regex::Regex;

    fn deref(&self) -> &Self::Target {
        let Regex(ref inner, PhantomData) = self;
        inner
    }
}

impl From<regex::Regex> for Regex {
    fn from(rx: regex::Regex) -> Self {
        Regex(rx, PhantomData)
    }
}

impl<Cfg> FromStr for Regex<Cfg>
where
    Cfg: RegexConfig,
{
    type Err = regex::Error;

    fn from_str(input: &str) -> StdResult<Self, Self::Err> {
        Self::try_from_str(input)
    }
}

/// This is a supporting trait for [`Regex`]. See that type's documentation for examples; see here
/// for method documentation.
///
/// TODO: Improve this description.
///
/// [`Regex`]: <struct.Regex.html>
pub trait RegexConfig {
    /// Returns a [`RegexBuilder`] that will parse the given string slice into a regex with this
    /// configuration if its [`build`] method immediately is called, although a caller also further
    /// could configure the [`RegexBuilder`] before calling [`build`].
    ///
    /// [`RegexBuilder`]: <https://docs.rs/regex/*/regex/struct.RegexBuilder.html>
    /// [`build`]: <https://docs.rs/regex/*/regex/struct.RegexBuilder.html#method.build>
    fn builder_from_str(input: &str) -> RegexBuilder;

    /// This must have the same effect as `builder_from_str`, but it may be more optimized because
    /// it can reuse the given [`String`]'s buffer.
    ///
    /// The default implementation simply passes a reference to the given [`String`] to
    /// `builder_from_str`.
    ///
    /// TODO: Add more such methods, for conversion from `Cow`,
    /// `inlinable_string::InlinableString`, `string_cache::Atom`, ....
    ///
    /// [`String`]: <https://doc.rust-lang.org/std/string/struct.String.html>
    fn builder_from_string(input: String) -> RegexBuilder {
        Self::builder_from_str(&input)
    }
}

pub mod config {
    use super::RegexConfig;
    use std::marker::PhantomData;

    /// This is the default [`regex`] configuration.
    ///
    /// [`regex`]: <https://docs.rs/regex/*/regex/>
    #[derive(Debug)]
    pub struct Standard;

    /// This configuration causes regexes to be wrapped in `\A(?:` and `)\z`, so that they will
    /// match a string only if they match the whole string, and not merely a substring of it.
    #[derive(Debug)]
    pub struct Anchored<Base = Standard>(PhantomData<Base>)
    where
        Base: RegexConfig;

    /// This configuration causes regexes to match without regard to letter case, by calling
    /// `rxb.case_insensitive(true)` where `rxb` is the relevant [`RegexBuilder`].
    ///
    /// [`RegexBuilder`]: <https://docs.rs/regex/*/regex/struct.RegexBuilder.html>
    #[derive(Debug)]
    pub struct CaseInsensitive<Base = Standard>(PhantomData<Base>)
    where
        Base: RegexConfig;

    /// This is unstable and longs for integer generics.
    ///
    /// TODO: Add `ProgramSizeLimit` (`size_limit`) and `RuntimeSizeLimit` (`dfa_size_limit`).
    #[derive(Debug)]
    pub struct SizeLimit<Base = Standard>(PhantomData<Base>)
    where
        Base: RegexConfig;
}

impl RegexConfig for config::Standard {
    fn builder_from_str(input: &str) -> RegexBuilder {
        RegexBuilder::new(input)
    }
}

impl<Base> RegexConfig for config::Anchored<Base>
where
    Base: RegexConfig,
{
    fn builder_from_str(input: &str) -> RegexBuilder {
        let anchor_extra_len = REGEX_ANCHOR_START.len() + REGEX_ANCHOR_END.len();

        let mut input_anchored = InlinableString::with_capacity(input.len() + anchor_extra_len);

        input_anchored.push_str(REGEX_ANCHOR_START);
        input_anchored.push_str(input);
        input_anchored.push_str(REGEX_ANCHOR_END);

        match input_anchored {
            InlinableString::Heap(s) => Base::builder_from_string(s),
            InlinableString::Inline(s) => Base::builder_from_str(&s),
        }
    }

    // TODO: implement optimized methods too.
}

impl<Base> RegexConfig for config::CaseInsensitive<Base>
where
    Base: RegexConfig,
{
    fn builder_from_str(input: &str) -> RegexBuilder {
        let mut rxb = Base::builder_from_str(input);
        rxb.case_insensitive(true);
        rxb
    }
}

impl<Base> RegexConfig for config::SizeLimit<Base>
where
    Base: RegexConfig,
{
    fn builder_from_str(input: &str) -> RegexBuilder {
        let mut rxb = Base::builder_from_str(input);
        rxb.size_limit(REGEX_SIZE_LIMIT);
        rxb.dfa_size_limit(REGEX_SIZE_LIMIT);
        rxb
    }
}

impl<'de, Cfg> Deserialize<'de> for Regex<Cfg>
where
    Cfg: RegexConfig,
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(RegexDeserializationVisitor(PhantomData))
    }
}

struct RegexDeserializationVisitor<Cfg>(PhantomData<Cfg>)
where
    Cfg: RegexConfig;

impl<'de, Cfg> serde::de::Visitor<'de> for RegexDeserializationVisitor<Cfg>
where
    Cfg: RegexConfig,
{
    type Value = Regex<Cfg>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a string that can be parsed into a regex")
    }

    fn visit_str<E>(self, input: &str) -> StdResult<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Self::Value::try_from_str(input).map_err(serde::de::Error::custom)
    }

    fn visit_string<E>(self, input: String) -> StdResult<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Self::Value::try_from_string(input).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use itertools::Itertools;
    use quickcheck::TestResult;

    trait RegexTrait {
        fn is_match(&self, input_text: &str) -> bool;

        fn captures_iter<'r, 't>(&'r self, input_text: &'t str) -> regex::CaptureMatches<'r, 't>;

        fn as_str(&self) -> &str;
    }

    impl RegexTrait for regex::Regex {
        fn is_match(&self, input_text: &str) -> bool {
            regex::Regex::is_match(self, input_text)
        }

        fn captures_iter<'r, 't>(&'r self, input_text: &'t str) -> regex::CaptureMatches<'r, 't> {
            regex::Regex::captures_iter(self, input_text)
        }

        fn as_str(&self) -> &str {
            regex::Regex::as_str(self)
        }
    }

    impl<Cfg> RegexTrait for Regex<Cfg>
    where
        Cfg: RegexConfig,
    {
        fn is_match(&self, input_text: &str) -> bool {
            regex::Regex::is_match(self, input_text)
        }

        fn captures_iter<'r, 't>(&'r self, input_text: &'t str) -> regex::CaptureMatches<'r, 't> {
            regex::Regex::captures_iter(self, input_text)
        }

        fn as_str(&self) -> &str {
            regex::Regex::as_str(self)
        }
    }

    fn test_regex_equivalence_for_input<Rx1, Rx2>(
        is_match_only: bool,
        rx1_build_result: StdResult<Rx1, regex::Error>,
        rx2_build_result: StdResult<Rx2, regex::Error>,
        input_text: &str,
    ) -> TestResult
    where
        Rx1: RegexTrait,
        Rx2: RegexTrait,
    {
        let rx1 = match rx1_build_result {
            Ok(rx) => rx,
            Err(_) => return TestResult::discard(),
        };
        let rx2 = match rx2_build_result {
            Ok(rx) => rx,
            Err(_) => return TestResult::discard(),
        };

        match (rx1.is_match(input_text), rx2.is_match(input_text)) {
            (false, false) | (true, true) => {}
            (false, true) => {
                return TestResult::error(format!(
                    "against {text:?}, regex {rx1:?} does not match but regex {rx2:?} matches",
                    text = input_text,
                    rx1 = rx1.as_str(),
                    rx2 = rx2.as_str()
                ));
            }
            (true, false) => {
                return TestResult::error(format!(
                    "against {text:?}, regex {rx1:?} matches but regex {rx2:?} does not match",
                    text = input_text,
                    rx1 = rx1.as_str(),
                    rx2 = rx2.as_str()
                ));
            }
        }

        if is_match_only {
            return TestResult::passed();
        }

        let rx1_captures_iter = rx1.captures_iter(input_text);
        let rx2_captures_iter = rx2.captures_iter(input_text);

        for (match_idx, (rx1_match, rx2_match)) in
            rx1_captures_iter.zip_eq(rx2_captures_iter).enumerate()
        {
            for (capture_idx, (rx1_capture, rx2_capture)) in
                rx1_match.iter().zip_eq(rx2_match.iter()).enumerate()
            {
                match (rx1_capture, rx2_capture) {
                    (None, None) => {}
                    (None, Some(_rx2_capture)) => {
                        panic!();
                    }
                    (Some(_rx1_capture), None) => {
                        panic!();
                    }
                    (Some(rx1_capture), Some(rx2_capture)) if (rx1_capture == rx2_capture) => {}
                    (Some(rx1_capture), Some(rx2_capture)) => {
                        return TestResult::error(format!(
                            "[match {match_idx}, capture group {capture_idx}] \
                             regex {rx1:?} matched {rx1_match:?} \
                             (start: {rx1_start}, end: {rx1_end}); \
                             regex {rx2:?} matched {rx2_match:?} \
                             (start: {rx2_start}, end: {rx2_end}).",
                            match_idx = match_idx,
                            capture_idx = capture_idx,
                            rx1 = rx1.as_str(),
                            rx1_match = rx1_capture.as_str(),
                            rx1_start = rx1_capture.start(),
                            rx1_end = rx1_capture.end(),
                            rx2 = rx2.as_str(),
                            rx2_match = rx2_capture.as_str(),
                            rx2_start = rx2_capture.start(),
                            rx2_end = rx2_capture.end()
                        ));
                    }
                }
            }
        }

        TestResult::passed()
    }

    // To run rustfmt on this code, temporarily change the `quickcheck! {...}` to `mod qc {...}`.
    // Beware, however, of rustfmt's adding trailing commas, which `quickcheck!` doesn't accept.
    quickcheck! {
        fn std_cfg_matches_regex_crate(pattern: String, haystack: String) -> TestResult {
            let theirs = regex::Regex::from_str(&pattern);
            let ours = Regex::<config::Standard>::try_from_string(pattern);

            test_regex_equivalence_for_input(false, ours, theirs, &haystack)
        }

        fn anchoring_basically_works(pattern: String, haystack: String) -> TestResult {
            let orig = match regex::Regex::from_str(&pattern) {
                Ok(rx) => rx,
                Err(_) => return TestResult::discard(),
            };

            let anchored = match Regex::<config::Anchored>::try_from_string(pattern) {
                Ok(rx) => rx,
                Err(_) => return TestResult::discard(),
            };

            let orig_matches = orig.is_match(&haystack);
            let orig_matches_whole_input = orig
                .find(&haystack)
                .map(|m| m.as_str() == haystack)
                .unwrap_or(false);
            let anchored_matches = anchored.is_match(&haystack);

            const F: bool = false;
            const T: bool = true;

            match (orig_matches, orig_matches_whole_input, anchored_matches) {
                (F, F, F) | (T, F, F) | (T, T, T) => TestResult::passed(),
                (F, F, T) => TestResult::error(
                    "plain regex doesn't match but anchored regex does"
                ),
                (T, F, T) => TestResult::error(
                    "plain regex matches only substring but anchored regex matches"
                ),
                (T, T, F) => TestResult::error(
                    "plain regex match matches whole input but anchored regex doesn't match"
                ),
                (F, T, F) | (F, T, T) => unreachable!("this is a bug in the crate `regex`"),
            }
        }

        fn anchoring_is_irrelevant_if_regex_does_not_match_anyway(
            pattern: String,
            haystack: String
        ) -> TestResult {
            let unanchored = match regex::Regex::from_str(&pattern) {
                Ok(rx) => rx,
                Err(_) => return TestResult::discard(),
            };

            if unanchored.is_match(&haystack) {
                return TestResult::discard();
            }

            let anchored = Regex::<config::Anchored>::try_from_string(pattern);

            test_regex_equivalence_for_input(false, Ok(unanchored), anchored, &haystack)
        }

        fn anchoring_is_irrelevant_if_regex_matches_whole_input(
            pattern: String,
            haystack: String
        ) -> TestResult {
            let unanchored = match regex::Regex::from_str(&pattern) {
                Ok(rx) => rx,
                Err(_) => return TestResult::discard(),
            };

            let matches_whole_input = unanchored
                .find(&haystack)
                .map(|m| m.as_str() == haystack)
                .unwrap_or(false);

            if !matches_whole_input {
                return TestResult::discard();
            }

            let anchored = Regex::<config::Anchored>::try_from_string(pattern);

            test_regex_equivalence_for_input(false, Ok(unanchored), anchored, &haystack)
        }

        fn anchoring_can_be_negated_for_nonempty_patterns(
            pattern: String,
            haystack: String
        ) -> TestResult {
            if pattern.is_empty() {
                return TestResult::discard();
            }

            let unchanged = regex::Regex::from_str(&pattern);

            let mut pattern = pattern;
            pattern.insert_str(0, "(?s:.*)(?:");
            pattern.push_str(")(?s:.*)");
            let unanchored_anchored = Regex::<config::Anchored>::try_from_string(pattern);

            test_regex_equivalence_for_input(true, unchanged, unanchored_anchored, &haystack)
        }
    }
}
