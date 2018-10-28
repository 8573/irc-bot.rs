use regex;
use regex::Regex;
use regex::RegexBuilder;
use std;

type RegexBuildResult = std::result::Result<Regex, regex::Error>;

/// Parses a `&str` into a case-insensitive `Regex`.
fn mk_case_insensitive_regex(s: &str) -> RegexBuildResult {
    let mut rx = RegexBuilder::new(s);
    rx.case_insensitive(true);
    rx.size_limit(1 << 17);
    rx.dfa_size_limit(1 << 17);
    rx.build()
}

/// This trait is implemented for `&str` and `Regex` such that one can pass either to certain
/// functions in this library, with a `&str` being parsed into a case-insensitive `Regex`, and a
/// `Regex` being accepted with its case-sensitivity unchanged.
pub trait IntoRegexCI {
    fn into_regex_ci(self) -> RegexBuildResult;
}

impl IntoRegexCI for Regex {
    fn into_regex_ci(self) -> RegexBuildResult {
        Ok(self)
    }
}

impl<'a> IntoRegexCI for &'a str {
    fn into_regex_ci(self) -> RegexBuildResult {
        mk_case_insensitive_regex(self)
    }
}
