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
