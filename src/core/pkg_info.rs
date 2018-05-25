use super::State;

fn choose(list: &'static [&'static str]) -> &'static str {
    list.iter()
        .find(|s| !s.is_empty() && !s.contains(char::is_control))
        .unwrap_or(&"<unknown>")
}

lazy_static! {
    pub(super) static ref NAME_STR: &'static str = choose(&[env!("CARGO_PKG_NAME")]);
    pub(super) static ref VERSION_STR: &'static str =
        choose(&[env!("IRC_BOT_RS_GIT_VERSION"), env!("CARGO_PKG_VERSION")]);
    pub(super) static ref HOMEPAGE_STR: &'static str = choose(&[env!("CARGO_PKG_HOMEPAGE")]);
    pub(super) static ref BRIEF_CREDITS_STRING: String = format!(
        "Built with <{url}> {ver}",
        url = HOMEPAGE_STR.deref(),
        ver = VERSION_STR.deref(),
    );
}

impl State {
    /// Returns a `&str` containing either the name of this crate or the text `"<unknown>"`.
    pub fn framework_crate_name(&self) -> &'static str {
        &NAME_STR
    }

    /// Returns a `&str` containing either version information for the bot framework or the text
    /// `"<unknown>"`.
    ///
    /// This version information is intended for display only, and is not necessarily in [SemVer]
    /// format or otherwise intended as machine-readable.
    ///
    /// [SemVer]: <https://semver.org>
    pub fn framework_version_str(&self) -> &'static str {
        &VERSION_STR
    }

    /// Returns a `&str` containing either a [Uniform Resource Locator (URL)][URI] for a Web page
    /// containing information about the bot framework, or the text `"<unknown>"`.
    ///
    /// [URI]: <https://tools.ietf.org/html/rfc3986>
    pub fn framework_homepage_url_str(&self) -> &'static str {
        &HOMEPAGE_STR
    }
}
