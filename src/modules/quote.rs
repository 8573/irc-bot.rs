// TODO: remove this
#![allow(unused)]

use clockpro_cache::ClockProCache;
use core::BotCmdAuthLvl as Auth;
use core::*;
use irc::client::data::User as AatxeUser;
use irc::client::prelude::Client as AatxeClient;
use itertools::Itertools;
use quantiles::ckms::CKMS;
use rando::Rando;
use ref_slice::ref_slice;
use regex::Regex;
use serde_yaml;
use smallbitvec::SmallBitVec;
use smallvec::SmallVec;
use std;
use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::iter;
use std::mem;
use std::num::ParseIntError;
use std::ops::Deref;
use std::str;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use string_cache::DefaultAtom;
use strum::IntoEnumIterator;
use try_map::FallibleMapExt;
use try_map::FlipResultExt;
use url::Url;
use url_serde::SerdeUrl;
use util;
use util::regex::IntoRegexCI;
use util::yaml::any_to_str;
use util::yaml::get_arg_by_short_or_long_key;
use util::yaml::iter_as_seq;
use util::yaml::scalar_to_str;
use util::yaml::str::YAML_STR_CMD;
use util::yaml::str::YAML_STR_ID;
use util::yaml::str::YAML_STR_R;
use util::yaml::str::YAML_STR_REGEX;
use util::yaml::str::YAML_STR_S;
use util::yaml::str::YAML_STR_STRING;
use util::yaml::str::YAML_STR_TAG;
use util::yaml::FW_SYNTAX_CHECK_FAIL;
use util::MustUse;
use walkdir::WalkDir;
use yaml_rust;
use yaml_rust::yaml::Hash as YamlHash;
use yaml_rust::Yaml;

#[cfg(test)]
use quickcheck as qc;

#[cfg(test)]
use url_serde::Serde;

/// This module provides functionality for retrieving quotations from a database thereof.
///
///
/// # The `quote` command
///
/// An IRC user is to interact with this module primarily via the bot command `quote`, which
/// requests a (pseudo-)random quotation from the bot's database of quotations.
///
/// ## Output
///
/// When a quotation is displayed, it will be prefaced with its hexadecimal **_identifier (ID)_**
/// in brackets. Quotation IDs _may_ simply be successive non-negative integers assigned in the
/// order in which the quotations were loaded, but one should not rely upon their being so.
/// Quotation IDs _may_ remain the same if the quotation database is reloaded (including when the
/// bot is restarted), but upon this too one should not rely.
///
/// If a quotation has been **abridged**, the abridgement will be indicated by placing curly
/// brackets (`{` and `}`) around the quotation ID. Otherwise, the brackets around the ID will be
/// square (`[` and `]`).
///
/// ## Input
///
/// The `quote` command takes as argument a YAML mapping, which may contain the following key-value
/// pairs (hereinafter termed _parameters_), listed by their keys:
///
/// - `regex` — The value of this parameter may be a string or a sequence of strings. If a string,
/// it will be parsed as a regular expression using the Rust [`regex`] library and [its particular
/// syntax][`regex` syntax]; if a sequence of strings, each string it contains will be parsed in
/// that manner. A quotation will be displayed only if it contains at least one match of each
/// regular expression so provided. Matches found in a quotation's tags count as matches found in
/// the quotation. These regular expressions will be matched case-insensitively by default;
/// however, this can be controlled with the [`regex` flag] `i`. This parameter is optional. This
/// parameter's key may be abbreviated as `r`.
///
/// - `string` — The value of this parameter may be a string or a sequence of strings. A quotation
/// will be displayed only if it contains at least one occurrence of each string so provided. These
/// strings will be matched case-sensitively. This parameter is optional. This parameter's key may
/// be abbreviated as `s`.
///
/// - `tag` — The value of this parameter may be a string or a sequence of strings. Each string so
/// provided will be interpreted as a quotation _tag_ (see below). A quotation will be displayed
/// only if it has all tags so provided. These tags will be matched case-sensitively. Note that
/// searching by `regex` also searches tags as well as quotations' text. This parameter is
/// optional.
///
/// - `id` — The value of this parameter should be a string. This parameter requests the quotation
/// whose ID, when displayed as described in the section "Output" above, is the value of this
/// parameter. Note that any asterisk suffixed to a quotation ID is not part of the quotation ID.
/// This parameter is optional.
///
/// - `anti-ping tactic` — The value of this parameter should be a string. This parameter overrides
/// the fields of the same name in the quotation database (see below). This parameter may be used
/// only by administrators of the bot. This parameter is optional.
///
/// ## Examples
///
/// ### `quote`
///
/// Request a pseudo-random quotation.
///
/// ### `quote s: rabbit`
///
/// Request a pseudo-random quotation that contains the text "rabbit".
///
/// ### `quote r: 'blue ?berr(y|ies)'`
///
/// Request a pseudo-random quotation that contains at least one of the following sequences of
/// text (without regard to letter case):
///
/// - "blueberry"
/// - "blue berry"
/// - "blueberries"
/// - "blue berries"
///
///
/// # Other commands
///
/// Other commands provided by this module include the following:
///
/// - `quote-database-info`
///
///
/// # Quotation files
///
/// The database of quotations from which the bot is to quote should be provided as a directory
/// named `quote` inside the bot's module data directory. This `quote` directory should contain
/// zero or more [YAML] files, termed _quotation files_, whose filenames do not start with the full
/// stop character (`.`). The text of each quotation file should constitute a YAML mapping with the
/// following key-value pairs (hereinafter termed _fields_):
///
/// - `channels` — The value of this field should be a string, which will be parsed as a regular
/// expression using the Rust [`regex`] library and [its particular syntax][`regex` syntax].
/// Quotations from this file will be shown only in channels whose names (including any leading
/// `#`) match this regular expression, unless an administrator of the bot chooses to override this
/// restriction. This regular expression will be prefixed with the anchor meta-character `^` and
/// suffixed with the anchor meta-character `$`, such that the regular expression must match the
/// whole of a channel name rather than only part of it. This field is **required**.
///
/// - `format` — The value of this field should be a string indicating the manner in which the
/// texts of the quotations in this file generally are formatted. This field is optional and
/// **defaults to `chat`**. For the allowed values, see the list of _quotation formats_ below.
///
/// - `anti-ping tactic` — The value of this field should be a string indicating the manner in
/// which the bot's operator wishes the bot to attempt to prevent people whose IRC nicknames appear
/// in this file's quotations from being "pinged" when those quotations are quoted. This field is
/// optional and defaults to `munge`. The allowed values are as follows:
///
///   - `none` — Have the bot not attempt not to ping people whose IRC nicknames appear in this
///   file's quotations. Be careful that the bot doesn't get banned from channels for annoying
///   people with frequent, unnecessary pings.
///
///   - `munge` — Have the bot alter the quotations' text in ways that are expected to be invisible
///   to most IRC users but also are expected to prevent most IRC users from getting pinged. As of
///   2017-06-07, this is known not to prevent some or all users of the chat platform Matrix from
///   being pinged.
///
///   - `eschew` — Simply forbid the bot from posting a quotation to a channel while one or more
///   users who would be expected to be pinged by the quotation are in the channel.
///
/// - `quotations` — The value of this field should be a sequence of _quotation records_. This
/// field is optional and defaults to an empty sequence.
///
/// Each _quotation record_ should be a mapping with the following fields:
///
/// - `format` — This field is optional and may be provided to override the file-level default set
/// in the quotation file's `format` field (see above), which itself **defaults to `chat`**. This
/// field allows the same values as the corresponding file-level field.
///
/// - `text` — The value of this field should be the text of the quotation. This field is
/// **required**.
///
/// - `URL` — The value of this field should be a string whose text forms a valid Uniform Resource
/// Locator (URL) that can be parsed as such by the Rust [`url`] library. If such a URL is
/// provided, it will be taken as a reference to a copy of the text of the quotation, such as in a
/// "pastebin" website, that may be offered rather than the quotation's text itself if that text is
/// too long to send in an IRC `PRIVMSG` in the relevant channel. This field is optional.
///
/// - `tags` — The value of this field should be a sequence of strings. These strings, termed
/// _tags_, count as part of the quotation for the purposes of the `quote` command's query
/// parameters, such as `regex` and `string`, but are not displayed with the quotation by default
/// (however, if an alternate display mode, such as posting quotations to "pastebin" websites, is
/// implemented, tags may be shown in that mode). E.g., if, in a quotation from IRC, someone is
/// using a different IRC nickname than usual, one could add the person's usual nickname to the
/// quotation as a tag, so that the quotation still can be returned when one searches the quotation
/// database for the person's usual nickname. This field is optional and defaults to an empty
/// sequence.
///
/// - `anti-ping tactic` — This field is optional and may be provided to override the file-level
/// default set in the quotation file's `anti-ping tactic` field (see above), which itself defaults
/// to `munge`. This field allows the same values as the corresponding file-level field.
///
/// ## Quotation formats
///
/// The following are the supported _quotation formats_:
///
/// - `chat` — This format is based on a stereotypical (Irssi-esque) plain-text Internet Relay Chat
/// log format. In this format, a quotation is expected to be given as lines of text each
/// representing a message sent by a specific user (or bot, or service), with the nickname of the
/// user in angle brackets (`<` and `>`) before the text of the message (or, for `/me` messages, an
/// asterisk followed by the nickname). Anything, such as a timestamp, before each line's first
/// "word" containing a (left *or right*) angle bracket or an asterisk will be treated as metadata
/// and not quoted by default, with a "word" defined as a sequence of characters that aren't ASCII
/// whitespace, followed by such whitespace (note that a line-break counts as whitespace). Any
/// leading whitespace or right angle brackets (`>`) similarly will not be quoted, so a right angle
/// bracket can be inserted to force the following text not to be treated as metadata. An example
/// of such a quotation's `text` field follows:
///
///   ```yaml
///   text: |
///     2018-08-27 21:16 <c74d> Why do you find solid mechanics harder than fluid mechanics?
///     2018-08-27 21:16 <c74d> Er, allow me to rephrase: Why do you find solid mechanics more difficult than fluid mechanics?
///   ```
///
///   Note that, in YAML, the `|` character here, the block scalar literal style indicator, means
///   that the line-breaks in the text will be preserved at the YAML level, although the `quote`
///   module will change them to spaces by default.
///
/// - `plain` — In this format, a quotation is treated as a plain, indivisible lump of text, not to
/// be parsed in any way, but only to be quoted whole. An example of such a quotation's `text`
/// field follows:
///
///   ```yaml
///   text: >
///     “I recognize that I am only making an assertion and
///     furnishing no proof; I am sorry, but this is a habit of
///     mine; sorry also that I am not alone in it; everybody
///     seems to have this disease.” — Mark Twain
///   ```
///
///   Note that, in YAML, the `>` character here, the block scalar folded style indicator, means
///   that the line-breaks in the text will be changed to spaces at the YAML level.
///
///
/// [`regex`]: <https://docs.rs/regex/*/regex/>
/// [`regex` syntax]: <https://docs.rs/regex/*/regex/#syntax>
/// [`regex` flag]: <https://docs.rs/regex/*/regex/#grouping-and-flags>
/// [`url`]: <https://docs.rs/url/*/url/>
/// [YAML]: <http://yaml.org>
pub fn mk() -> Module {
    mk_module("quote")
        .on_load(Box::new(on_load))
        .command(
            "quote",
            "{regex: '[...]', string: '[...]', tag: '[...]', id: '[ID]'}",
            "Request a quotation from the bot's database of quotations. For usage instructions, \
             see the full documentation: \
             <https://docs.rs/irc-bot/*/irc_bot/modules/fn.quote.html>.",
            Auth::Public,
            Box::new(quote),
            &[],
        ).command(
            "quote-database-info",
            "",
            "Request information about the bot's database of quotations, such as the number of \
             quotations in the database.",
            Auth::Public,
            Box::new(show_qdb_info),
            &[],
        ).command(
            "quote-database-reload",
            "",
            "Tell the bot to reload its quotation database.",
            Auth::Admin,
            Box::new(reload_qdb),
            &[],
        ).end()
}

lazy_static! {
    static ref QDB: RwLock<QuotationDatabase> = RwLock::new(QuotationDatabase::new());
    static ref YAML_STR_ANTI_PING_TACTIC: Yaml = util::yaml::mk_str("anti-ping tactic");
}

#[derive(Debug)]
struct QuotationDatabase {
    files: SmallVec<[QuotationFileMetadata; 8]>,

    quotations: Vec<Quotation>,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct QuotationFileId(usize);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
struct QuotationId(usize);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
struct QuotationFileIR {
    channels: String,

    #[serde(default = "default_quotation_format_for_serde")]
    format: QuotationFormat,

    #[serde(default = "default_anti_ping_tactic_for_serde")]
    #[serde(rename = "anti-ping tactic")]
    anti_ping_tactic: AntiPingTactic,

    #[serde(default)]
    quotations: Vec<QuotationIR>,
}

#[derive(Debug)]
struct QuotationFileMetadata {
    name: String,

    file_id: QuotationFileId,

    channels_regex: Regex,

    quotation_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
struct QuotationIR {
    #[serde(default)]
    format: Option<QuotationFormat>,

    text: String,

    #[serde(default)]
    tags: SmallVec<[DefaultAtom; 2]>,

    #[serde(default)]
    #[serde(rename = "URL")]
    url: Option<SerdeUrl>,

    #[serde(default)]
    anti_ping_tactic: Option<AntiPingTactic>,
}

#[cfg_attr(test, derive(Clone))]
#[derive(Debug)]
struct Quotation {
    id: QuotationId,

    file_id: QuotationFileId,

    format: QuotationFormat,

    text: String,

    tags: SmallVec<[DefaultAtom; 2]>,

    url: Option<SerdeUrl>,

    anti_ping_tactic: AntiPingTactic,
}

#[derive(Copy, Clone, Debug, Deserialize, EnumIter, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
enum QuotationFormat {
    Chat,
    Plain,
}

fn default_quotation_format_for_serde() -> QuotationFormat {
    QuotationFormat::Chat
}

#[derive(Copy, Clone, Debug, Deserialize, EnumIter, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
enum AntiPingTactic {
    Munge,
    Eschew,
    None,
}

fn default_anti_ping_tactic_for_serde() -> AntiPingTactic {
    AntiPingTactic::Munge
}

#[derive(Debug)]
enum QuotationChoice<'q> {
    /// Reply with the text of the quotation.
    Text {
        quotation: &'q Quotation,
        // variant_id: usize,
        // TODO: ^
    },

    /// Reply with the URL of the quotation.
    Url {
        quotation_id: QuotationId,
        url: &'q Url,
    },
}

impl QuotationDatabase {
    fn new() -> Self {
        QuotationDatabase {
            files: Default::default(),
            quotations: Default::default(),
        }
    }

    fn get_file_metadata_by_id(&self, id: QuotationFileId) -> Option<&QuotationFileMetadata> {
        self.files.get(id.array_index())
    }

    fn get_quotation_by_id(&self, id: QuotationId) -> Option<&Quotation> {
        self.quotations.get(id.array_index())
    }
}

fn quote(
    state: &State,
    request_metadata: &MsgMetadata,
    arg: &Yaml,
) -> std::result::Result<Reaction, BotCmdResult> {
    let params = prepare_quote_params(state, request_metadata, arg)?;
    let reply_dest = state.guess_reply_dest(request_metadata)?;
    let qdb = read_qdb()?;
    let channel_users = state.read_aatxe_client(reply_dest.server_id, |aatxe_client| {
        Ok(aatxe_client
            .list_users(reply_dest.target)
            .unwrap_or_default())
    })?;

    let output_text = match pick_quotation(
        state,
        request_metadata,
        &params,
        reply_dest,
        &qdb,
        &channel_users,
    ) {
        Ok(QuotationChoice::Text { quotation }) => {
            render_quotation(&params, quotation, &channel_users)?.into()
        }
        Ok(QuotationChoice::Url { quotation_id, url }) => {
            format!("[{id}] <{url}>", id = quotation_id, url = url).into()
        }
        Err(msg) => return Err(msg),
    };

    Ok(Reaction::Msg(output_text))
}

#[derive(Debug, Default)]
struct QuoteParams<'a> {
    // TODO: Use `RegexSet`.
    regexes: SmallVec<[Regex; 8]>,
    literals: SmallVec<[Cow<'a, str>; 8]>,
    tags: SmallVec<[Cow<'a, str>; 4]>,
    id: Option<Cow<'a, str>>,
    anti_ping_tactic: Option<AntiPingTactic>,
}

// TODO: Add a parameter controlling whether quotations may be abridged.
fn prepare_quote_params<'arg>(
    state: &State,
    request_metadata: &MsgMetadata,
    arg: &'arg Yaml,
) -> std::result::Result<QuoteParams<'arg>, BotCmdResult> {
    let arg = arg.as_hash().expect(FW_SYNTAX_CHECK_FAIL);
    let admin_param_keys = [&YAML_STR_ANTI_PING_TACTIC];
    let first_admin_param_used = admin_param_keys.iter().find(|k| arg.get(k).is_some());

    if let Some(admin_param_key) = first_admin_param_used {
        if !state.have_admin(request_metadata.prefix)? {
            return Err(BotCmdResult::ParamUnauthorized(any_to_str(
                admin_param_key,
                Cow::Borrowed,
            )?));
        }
    }

    let regexes = iter_as_seq(get_arg_by_short_or_long_key(
        arg,
        &YAML_STR_R,
        &YAML_STR_REGEX,
    )?).map(|y| {
        scalar_to_str(
            y,
            Cow::Borrowed,
            "a search term given in the argument `regex`",
        ).map_err(Into::into)
    }).map_results(|s| s.as_ref().into_regex_ci().map_err(Into::into))
    .collect::<Result<Result<_>>>()??;

    let literals = iter_as_seq(get_arg_by_short_or_long_key(
        arg,
        &YAML_STR_S,
        &YAML_STR_STRING,
    )?).map(|y| {
        scalar_to_str(
            y,
            Cow::Borrowed,
            "a search term given in the argument `string`",
        ).map_err(Into::into)
    }).collect::<Result<_>>()?;

    let tags = iter_as_seq(arg.get(&YAML_STR_TAG))
        .map(|y| {
            scalar_to_str(
                y,
                Cow::Borrowed,
                "a search term given in the argument `tag`",
            ).map_err(Into::into)
        }).collect::<Result<_>>()?;

    let id = arg
        .get(&YAML_STR_ID)
        .try_map(|y| scalar_to_str(y, Cow::Borrowed, "the argument `id`"))?;

    let anti_ping_tactic = arg
        .get(&YAML_STR_ANTI_PING_TACTIC)
        .try_map(|y| scalar_to_str(y, Cow::Borrowed, "the argument `anti-ping tactic`"))?
        .try_map(|s: Cow<'arg, str>| serde_yaml::from_str(&s))?;

    Ok(QuoteParams {
        regexes,
        literals,
        tags,
        id,
        anti_ping_tactic,
    })
}

// TODO: Probabilities
fn pick_quotation<'q>(
    state: &State,
    request_metadata: &MsgMetadata,
    arg: &QuoteParams,
    reply_dest: MsgDest,
    qdb: &'q QuotationDatabase,
    channel_users: &[AatxeUser],
) -> std::result::Result<QuotationChoice<'q>, BotCmdResult> {
    let reply_content_max_len = state.privmsg_content_max_len(reply_dest)?;

    let quotations = match arg.id {
        Some(ref requested_quotation_id) => ref_slice(get_quotation_by_user_specified_id(
            qdb,
            requested_quotation_id,
        )?),
        None => &qdb.quotations,
    };

    let file_permissions = check_file_permissions(qdb, reply_dest);

    let mut rejected_a_quotation_for_length = false;

    quotations
        .rand_iter()
        .filter_map(
            |quotation: &'q Quotation| -> Option<Result<QuotationChoice>> {
                match (|quotation: &'q Quotation| -> Result<Option<QuotationChoice>> {
                    if !quotation_matches_query_params(arg, quotation)? {
                        return Ok(None);
                    }

                    if file_permissions.get(quotation.file_id.array_index()) != Some(true) {
                        return Ok(None);
                    }

                    // TODO: Pick a random variant that satisfies query parameters

                    // If the quotation is too long to post to this channel in a single `PRIVMSG`,
                    // post its URL if it has one, or try a different quotation otherwise.
                    //
                    // Now, it's possible that even the URL wouldn't fit in one `PRIVMSG`. Perhaps
                    // something should be done about that.
                    if rendered_quotation_byte_len(quotation) > reply_content_max_len {
                        return match quotation.url {
                            Some(ref url) => Ok(Some(QuotationChoice::Url {
                                quotation_id: quotation.id,
                                url,
                            })),
                            None => {
                                rejected_a_quotation_for_length = true;
                                Ok(None)
                            }
                        };
                    }

                    if arg.anti_ping_tactic.unwrap_or(quotation.anti_ping_tactic)
                        == AntiPingTactic::Eschew
                        && quotation_text_contains_any_nick(quotation, channel_users)
                    {
                        return Ok(None);
                    }

                    Ok(Some(QuotationChoice::Text { quotation }))
                })(quotation)
                {
                    Ok(Some(q)) => Some(Ok(q)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            },
        ).next()
        .flip()?
        .ok_or_else(|| {
            Reaction::Reply(
                if rejected_a_quotation_for_length {
                    "I have found one or more quotations matching the given query parameters in \
                     the files I am allowed to quote in this channel, but all such quotations \
                     were too long to quote safely in this channel."
                } else {
                    "I have found no quotation matching the given query parameters in the files I \
                     am allowed to quote in this channel."
                }.into(),
            ).into()
        })
}

fn render_quotation(
    arg: &QuoteParams,
    quotation: &Quotation,
    channel_users: &[AatxeUser],
) -> Result<String> {
    let mut output_text_pieces = Default::default();

    let MustUse(text_was_abridged) =
        append_quotation_text_pieces(&mut output_text_pieces, arg, quotation, channel_users)?;

    let (pre_id_bracket, post_id_bracket) = if text_was_abridged {
        ("{", "}")
    } else {
        ("[", "]")
    };

    Ok(format!(
        "{pre_id_bracket}{id}{post_id_bracket} {text}",
        id = quotation.id,
        text = output_text_pieces.into_iter().format(""),
        pre_id_bracket = pre_id_bracket,
        post_id_bracket = post_id_bracket,
    ))
}

/// Appends the pieces of the given quotation's text to `buf`, applying anti-ping tactics, and
/// returns whether the quotation is considered to have been abridged in the process.
///
/// The pieces are to be concatenated when one is done processing them; to avoid needless
/// allocation, this intermediate step declines to do so.
///
/// # Panics
///
/// The anti-ping tactic `Eschew` should be handled before calling this function. If the given
/// quotation's anti-ping tactic is `Eschew` and the nickname of a user the bot believes to be in
/// the destination channel appears in the quotation's text, a debug assertion may fail.
fn append_quotation_text_pieces<'q>(
    buf: &mut SmallVec<[&'q str; 64]>,
    arg: &QuoteParams,
    quotation: &'q Quotation,
    channel_users: &[AatxeUser],
) -> Result<MustUse<bool>> {
    for_each_quotation_text_piece(arg, quotation, channel_users, |s| buf.push(s))
}

fn for_each_quotation_text_piece<'q, 'arg, 'users, F>(
    arg: &QuoteParams<'arg>,
    quotation: &'q Quotation,
    channel_users: &'users [AatxeUser],
    mut f: F,
) -> Result<MustUse<bool>>
where
    F: FnMut(&'q str) -> (),
{
    let anti_ping_tactic = arg.anti_ping_tactic.unwrap_or(quotation.anti_ping_tactic);

    match quotation.format {
        QuotationFormat::Chat => {
            let orig_line_count = quotation.text.lines().count();
            let mut output_line_count = 0;
            let lines = chat_lines_stripped(quotation);

            {
                let text = lines
                    .map(|line| {
                        // Panics here will be caught and are acceptable, and having more than
                        // `usize::MAX` lines is most unlikely anyway.
                        output_line_count += 1;
                        line
                    })
                    // TODO: Try using two spaces between lines if that fits.
                    // TODO: Make the line separator configurable.
                    .intersperse(" ");

                match anti_ping_tactic {
                    AntiPingTactic::Munge => text
                        .flat_map(|s| munge_user_nicks(s, channel_users))
                        .for_each(f),
                    AntiPingTactic::Eschew => {
                        debug_assert!(!quotation_text_contains_any_nick(quotation, channel_users));
                        text.for_each(f)
                    }
                    AntiPingTactic::None => text.for_each(f),
                }
            }

            Ok(MustUse(output_line_count != orig_line_count))
        }
        QuotationFormat::Plain => {
            let text = &quotation.text;

            match anti_ping_tactic {
                AntiPingTactic::Munge => munge_user_nicks(text, channel_users).for_each(f),
                AntiPingTactic::Eschew => {
                    debug_assert!(!quotation_text_contains_any_nick(quotation, channel_users));
                    f(text)
                }
                AntiPingTactic::None => f(text),
            }

            Ok(MustUse(false))
        }
    }
}

// #[derive(Debug)]
// struct QuotationTextPieces<'q, 'arg, 'users> {
//     arg: &'arg yaml::yaml::Hash,
//     channel_users: &'users [AatxeUser],
//     inner: QuotationTextPiecesInner,
//     abridged: bool,
// }

// #[derive(Debug)]
// enum QuotationTextPiecesInner<'q> {
//     Chat {
//         lines: ChatLinesStripped<'q>,
//         orig_line_count: usize,
//     },
//     Plain {
//         quotation: &'q Quotation,
//     },
// }

// impl<'q, 'arg, 'users> Iterator for QuotationTextPieces<'q, 'arg, 'users> {
//     fn next(&mut self) -> Option<&'q str> {}
// }

fn munge_user_nicks<'a, 'u>(s: &'a str, users: &'u [AatxeUser]) -> util::Munge<'a> {
    util::zwsp_munge(s, users.iter().map(|user| user.get_nickname()))
}

/// Returns a tuple of (0) an iterator over the lines of the given `chat`-format quotation's text,
/// stripped of metadata and leading and trailing whitespace; and (1) a Boolean value indicating
/// whether this stripping is considered to constitute abridging the quotation.
///
/// "Metadata" is considered to comprise (1) anything in each line before the first "word" (defined
/// as in the bot module documentation comment above) to contain a left *or right* angle bracket or
/// asterisk, and (2) any leading *right* angle brackets remaining after such metadata is stripped.
/// If a line contains no angle bracket or asterisk, or this stripping process otherwise yields an
/// empty line, then the whole line will be discarded. If one or more whole lines are discarded,
/// the quotation is considered to have been abridged.
///
/// # Panics
///
/// This function includes a debug assertion that the given quotation really is in the `chat`
/// format.
fn chat_lines_stripped(quotation: &Quotation) -> impl Iterator<Item = &str> + Clone {
    debug_assert_eq!(quotation.format, QuotationFormat::Chat);

    strip_quotation_lines(quotation, strip_chat_metadata)
}

fn strip_chat_metadata(line: &str) -> Option<&str> {
    lazy_static! {
        static ref METADATA_REGEX: Regex = Regex::new("^(?:[^[:space:]*<>]+(?:[[:space:]]+|$))*")
            .expect("Apparently, we have a syntax error in a static regex.");
    }

    METADATA_REGEX
        .find(line)
        .and_then(|regex_match| line.get(regex_match.end()..))
        .map(|line| line.trim_left_matches(">"))
}

fn strip_quotation_lines<F>(
    quotation: &Quotation,
    filter_map: F,
) -> impl Iterator<Item = &str> + Clone
where
    F: Fn(&str) -> Option<&str> + Clone,
{
    quotation
        .text
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .filter_map(filter_map)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
}

/// Returns whether any of the given users' nicknames appear in the given quotation's text.
fn quotation_text_contains_any_nick<'u, I>(quotation: &Quotation, users: I) -> bool
where
    I: IntoIterator<Item = &'u AatxeUser>,
{
    quotation_text_contains_any(quotation, users.into_iter().map(|user| user.get_nickname()))
}

/// Returns whether any of the given `needles` appear in the given quotation's text.
fn quotation_text_contains_any<'a, I>(quotation: &Quotation, needles: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    let mut needles = needles.into_iter();

    match quotation.format {
        QuotationFormat::Chat => needles
            .cartesian_product(chat_lines_stripped(quotation))
            .any(|(needle, line)| line.contains(needle)),
        QuotationFormat::Plain => needles.any(|needle| quotation.text.contains(needle)),
    }
}

fn quotation_matches_query_params(
    QuoteParams {
        ref regexes,
        ref literals,
        ref tags,
        id: _,
        anti_ping_tactic: _,
    }: &QuoteParams,
    quotation: &Quotation,
) -> Result<bool> {
    #[derive(Debug, Eq, PartialEq)]
    enum Status {
        NotAllMatchesFound,
        AllMatchesFound,
    }

    // Make sure that the quotation has all the requested tags.
    if !tags.iter().all(|tag_wanted| {
        quotation
            .tags
            .iter()
            .any(|tag_found| tag_found == tag_wanted.as_ref())
    }) {
        return Ok(false);
    }

    // These bit vectors record whether a match for each search term has been found in the
    // quotation's text.
    let mut regexes_matched = SmallBitVec::from_elem(regexes.len(), false);
    let mut literals_matched = SmallBitVec::from_elem(literals.len(), false);

    // This function searches for the search terms (which do not include requested tags) in the
    // given text, marks any it finds as matched, and returns whether all the search terms have
    // been matched.
    let mut check_all_search_terms = |haystack| {
        check_search_terms(regexes, &mut regexes_matched, |regex| {
            regex.is_match(haystack)
        });
        check_search_terms(literals, &mut literals_matched, |literal| {
            haystack.contains(literal.as_ref())
        });

        if regexes_matched.all_true() && literals_matched.all_true() {
            Status::AllMatchesFound
        } else {
            Status::NotAllMatchesFound
        }
    };

    fn check_search_terms<T, I, F>(search_terms: I, matched: &mut SmallBitVec, predicate: F)
    where
        I: IntoIterator<Item = T>,
        F: Fn(T) -> bool,
    {
        for (index, search_term) in search_terms.into_iter().enumerate() {
            if matched.get(index) == Some(true) {
                // Only check the search terms for which matches have not yet been found.
                continue;
            }
            if predicate(search_term) {
                matched.set(index, true);
            }
        }
    }

    // Search for the search terms in the quotation's text.
    match quotation.format {
        QuotationFormat::Chat => {
            for line in chat_lines_stripped(quotation) {
                if check_all_search_terms(line) == Status::AllMatchesFound {
                    return Ok(true);
                }
            }
        }
        QuotationFormat::Plain => {
            if check_all_search_terms(&quotation.text) == Status::AllMatchesFound {
                return Ok(true);
            }
        }
    }

    // Search for the search terms in the quotation's tags.
    for tag in &quotation.tags {
        if check_all_search_terms(tag) == Status::AllMatchesFound {
            return Ok(true);
        }
    }

    Ok(false)
}

fn quotation_byte_len(quotation: &Quotation) -> usize {
    match quotation.format {
        QuotationFormat::Chat => {
            chat_lines_stripped(quotation)
                // Add 1 here to account for the space that will be added between each line.
                .map(|s| s.len() + 1)
                // Sum the lengths of the lines.
                .sum::<usize>()
                // Subtract 1 here to account for the first line not coming after another line,
                // using `saturating_sub` so that, if there are *no* lines, the total will remain
                // at 0 rather than overflowing.
                .saturating_sub(1)
        }
        QuotationFormat::Plain => quotation.text.len(),
    }
}

/// Returns an upper bound on the length in bytes of the rendered form of the given quotation's
/// text.
fn rendered_quotation_byte_len(quotation: &Quotation) -> usize {
    quotation_byte_len(quotation) + {
        // Account for the ID prefix, which has the form "[N] ", with `N` being the quotation's
        // ID's `Display` representation. Using the actual `Display` implementation of
        // `QuotationId` (via `ToString`) seems, though inefficient, the safest method of
        // determining the length of that representation, especially to defend against possible
        // changes in the `Display` implementation of `QuotationId`.
        3 + quotation.id.to_string().len()
    }
}

/// Computes whether the given message destination is allowed to see the quotations in each of our
/// quotation files.
///
/// This function's return value is such that, with `file: QuotationFileMetadata`,
/// `check_file_permissions(qdb, msg_dest).get(file.array_index())` is `Some(true)` if and only if
/// the message destination `msg_dest` is allowed to see `file`'s quotations. In actual usage, this
/// function's return value should be saved and not recomputed for each quotation file.
///
/// It is assumed that checking permissions for each file is more efficient than doing so for each
/// candidate quotation, as there are expected to be few files and many quotations.
fn check_file_permissions(
    QuotationDatabase { files, .. }: &QuotationDatabase,
    MsgDest { server_id, target }: MsgDest,
) -> SmallBitVec {
    // TODO: Account for the server as well as the channel, with a `servers` field in the quotation
    // files.

    let mut result = SmallBitVec::from_elem(files.len(), false);

    for (index, file) in files.iter().enumerate() {
        result.set(index, file.channels_regex.is_match(target));
    }

    result
}

fn get_quotation_by_user_specified_id<'q, 'arg>(
    qdb: &'q QuotationDatabase,
    requested_quotation_id_str: &Cow<'arg, str>,
) -> std::result::Result<&'q Quotation, BotCmdResult> {
    match requested_quotation_id_str
        .parse()
        .map(|quotation_id| qdb.get_quotation_by_id(quotation_id))
    {
        Ok(Some(quotation)) => Ok(quotation),
        Ok(None) => Err(BotCmdResult::UserErrMsg(
            format!(
                "The given value of the parameter `id`, {input:?}, was not recognized as \
                 the identifier of a quotation in my quotation database.",
                input = requested_quotation_id_str,
            ).into(),
        )),
        Err(parse_err) => Err(BotCmdResult::UserErrMsg(
            format!(
                "The given value of the parameter `id`, {input:?}, failed to parse as a \
                 quotation identifier: {parse_err}",
                input = requested_quotation_id_str,
                parse_err = parse_err,
            ).into(),
        )),
    }
}

fn show_qdb_info(state: &State, request_metadata: &MsgMetadata, _: &Yaml) -> Result<Reaction> {
    let qdb = read_qdb()?;
    let reply_dest = state.guess_reply_dest(request_metadata)?;
    let file_permissions = check_file_permissions(&qdb, reply_dest);
    let any_files_are_visible = !file_permissions.is_empty() && !file_permissions.all_false();

    Ok(Reaction::Msgs(
        vec![
            format!(
                "I have {quotation_qty} total quotation(s) in {file_qty} file(s). \
                 The files I may name in this channel, along with their quotation counts, are: \
                 {file_list}.",
                quotation_qty = qdb.quotations.len(),
                file_qty = qdb.files.len(),
                file_list = qdb
                    .files
                    .iter()
                    .filter(|file| file_permissions.get(file.array_index()) == Some(true))
                    .map(|file| format!(
                        "{name} ({quotation_count})",
                        name = file.name,
                        quotation_count = file.quotation_count
                    )).pad_using(1, |_| "<none>".to_owned())
                    .format(", "),
            ).into(),
        ].into(),
    ))
}

fn reload_qdb(state: &State, _: &MsgMetadata, _: &Yaml) -> Result<Reaction> {
    on_load(state)?;

    let qdb = read_qdb()?;

    let chat_text_pieces_5ns = {
        let mut quantiles = CKMS::new(0.0001);
        for quotation in &qdb.quotations {
            if quotation.format == QuotationFormat::Chat {
                let mut text_piece_qty: u32 = 0;
                for_each_quotation_text_piece(&Default::default(), quotation, &[], |_| {
                    text_piece_qty = text_piece_qty.saturating_add(1)
                });
                quantiles.insert(text_piece_qty)
            }
        }
        [0.0, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .filter_map(|&q| quantiles.query(q).map(|(_, r)| r))
            .collect::<SmallVec<[_; 5]>>()
    };

    // TODO: Also report a 5NS for the byte-lengths of quotations.
    Ok(Reaction::Msg(
        format!(
            "I have reloaded my quotation database. The five-number summary of the numbers of \
             pieces into which chat-format quotations' texts get broken, assuming no anti-ping \
             munging, is {chat_text_pieces_5ns:?}.",
            chat_text_pieces_5ns = chat_text_pieces_5ns,
        ).into(),
    ))
}

fn read_qdb() -> Result<impl Deref<Target = QuotationDatabase>> {
    match QDB.read() {
        Ok(guard) => Ok(guard),
        Err(_guard) => Err(ErrorKind::LockPoisoned("quotation database".into()).into()),
    }
}

fn on_load(state: &State) -> Result<()> {
    let data_path = state.module_data_path()?.join("quote");

    if !data_path.exists() {
        debug!("No quotation database found; not loading quotation database.");
        return Ok(());
    }

    let mut old_qdb = match QDB.write() {
        Ok(guard) => guard,
        Err(_guard) => return Err(ErrorKind::LockPoisoned("quotation database".into()).into()),
    };
    let mut new_qdb = QuotationDatabase::new();

    // Reuse any memory already allocated for an old quotation database.
    mem::swap(&mut old_qdb.files, &mut new_qdb.files);
    mem::swap(&mut old_qdb.quotations, &mut new_qdb.quotations);
    new_qdb.files.clear();
    new_qdb.quotations.clear();

    let mut next_quotation_id = 0;

    for entry in WalkDir::new(data_path)
        .follow_links(true)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_entry(|entry| {
            entry.file_type().is_file() && !entry.file_name().to_string_lossy().starts_with(".")
        }) {
        let entry = entry?;
        let path = entry.path();
        trace!("Loading quotation file: {}", path.display());

        let QuotationFileIR {
            channels: mut file_channels_regex,
            format: file_default_format,
            anti_ping_tactic: file_default_anti_ping_tactic,
            quotations: deserialized_quotations,
        } = serde_yaml::from_reader(BufReader::new(File::open(path)?))?;

        let file_id = QuotationFileId(new_qdb.files.len());

        let file_metadata = QuotationFileMetadata {
            name: entry.file_name().to_string_lossy().into_owned(),
            file_id,
            channels_regex: {
                file_channels_regex.reserve_exact(2);
                file_channels_regex.insert_str(0, "^(?:");
                file_channels_regex.push_str(")$");
                file_channels_regex.into_regex_ci()?
            },
            quotation_count: deserialized_quotations.len(),
        };

        new_qdb.files.push(file_metadata);

        debug_assert_eq!(next_quotation_id, new_qdb.quotations.len());

        // Make sure that loading this quotation file will not cause integer overflow in the number
        // of quotations.
        if next_quotation_id
            .checked_add(deserialized_quotations.len())
            .is_none()
        {
            return Err(ErrorKind::IntegerOverflow(
                "Attempted to load a quotation database containing too many quotations.".into(),
            ).into());
        }

        new_qdb
            .quotations
            .extend(
                deserialized_quotations
                    .into_iter()
                    .map(|deserialized_quotation| {
                        let QuotationIR {
                            format,
                            text,
                            mut tags,
                            url,
                            anti_ping_tactic,
                        } = deserialized_quotation;

                        Quotation {
                            id: {
                                let id = next_quotation_id;
                                // We already have checked for possible overflow, above.
                                next_quotation_id += 1;
                                QuotationId(id)
                            },
                            file_id,
                            format: format.unwrap_or(file_default_format),
                            text,
                            tags: {
                                tags.sort_unstable();
                                tags
                            },
                            url,
                            anti_ping_tactic: anti_ping_tactic
                                .unwrap_or(file_default_anti_ping_tactic),
                        }
                    }),
            );
    }

    *old_qdb = new_qdb;

    debug!("Finished loading quotation database.");

    Ok(())
}

impl QuotationFileMetadata {
    fn array_index(&self) -> usize {
        self.file_id.array_index()
    }
}

impl QuotationFileId {
    fn array_index(&self) -> usize {
        let &QuotationFileId(inner) = self;
        inner
    }
}

impl QuotationId {
    fn array_index(&self) -> usize {
        let &QuotationId(inner) = self;
        inner
    }
}

impl fmt::Display for QuotationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let &QuotationId(id_number) = self;
        write!(f, "{id_number:X}", id_number = id_number)
    }
}

impl str::FromStr for QuotationId {
    type Err = ParseIntError;
    fn from_str(src: &str) -> std::result::Result<Self, ParseIntError> {
        Ok(QuotationId(usize::from_str_radix(src, 16)?))
    }
}

#[cfg(test)]
impl qc::Arbitrary for Quotation {
    fn arbitrary<G>(g: &mut G) -> Self
    where
        G: qc::Gen,
    {
        Quotation {
            id: qc::Arbitrary::arbitrary(g),
            file_id: qc::Arbitrary::arbitrary(g),
            format: qc::Arbitrary::arbitrary(g),
            text: qc::Arbitrary::arbitrary(g),
            tags: <Vec<String> as qc::Arbitrary>::arbitrary(g)
                .into_iter()
                .map(Into::into)
                .collect(),
            url: <String as qc::Arbitrary>::arbitrary(g)
                .parse()
                .ok()
                .map(Serde),
            anti_ping_tactic: qc::Arbitrary::arbitrary(g),
        }
    }

    // TODO: Implement `shrink` (see below).
}

// TODO: `derive` this `Arbitrary` implementation if QuickCheck implements such a `derive` (see
// <https://github.com/BurntSushi/quickcheck/issues/98>).
#[cfg(test)]
impl qc::Arbitrary for QuotationId {
    fn arbitrary<G>(g: &mut G) -> Self
    where
        G: qc::Gen,
    {
        QuotationId(qc::Arbitrary::arbitrary(g))
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        let QuotationId(inner) = self;
        Box::new(qc::Arbitrary::shrink(inner).map(QuotationId))
    }
}

// TODO: `derive` this `Arbitrary` implementation if QuickCheck implements such a `derive` (see
// <https://github.com/BurntSushi/quickcheck/issues/98>).
#[cfg(test)]
impl qc::Arbitrary for QuotationFileId {
    fn arbitrary<G>(g: &mut G) -> Self
    where
        G: qc::Gen,
    {
        QuotationFileId(qc::Arbitrary::arbitrary(g))
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        let QuotationFileId(inner) = self;
        Box::new(qc::Arbitrary::shrink(inner).map(QuotationFileId))
    }
}

// TODO: `derive` this `Arbitrary` implementation if QuickCheck implements such a `derive` (see
// <https://github.com/BurntSushi/quickcheck/issues/98>).
#[cfg(test)]
impl qc::Arbitrary for QuotationFormat {
    fn arbitrary<G>(g: &mut G) -> Self
    where
        G: qc::Gen,
    {
        *g.choose(&QuotationFormat::iter().collect::<SmallVec<[_; 8]>>())
            .unwrap()
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        match self {
            QuotationFormat::Chat => qc::single_shrinker(QuotationFormat::Plain),
            QuotationFormat::Plain => qc::empty_shrinker(),
        }
    }
}

// TODO: `derive` this `Arbitrary` implementation if QuickCheck implements such a `derive` (see
// <https://github.com/BurntSushi/quickcheck/issues/98>).
#[cfg(test)]
impl qc::Arbitrary for AntiPingTactic {
    fn arbitrary<G>(g: &mut G) -> Self
    where
        G: qc::Gen,
    {
        *g.choose(&AntiPingTactic::iter().collect::<SmallVec<[_; 8]>>())
            .unwrap()
    }

    fn shrink(&self) -> Box<Iterator<Item = Self>> {
        match self {
            AntiPingTactic::Munge => qc::single_shrinker(AntiPingTactic::Eschew),
            AntiPingTactic::Eschew => qc::single_shrinker(AntiPingTactic::None),
            AntiPingTactic::None => qc::empty_shrinker(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::TestResult;

    // To run rustfmt on this code, temporarily change the `quickcheck! {...}` to `mod qc {...}`.
    // Beware, however, of rustfmt adding trailing commas, which `quickcheck!` doesn't accept.
    quickcheck! {
        fn quotation_id_string_roundtrip_conv_1(original: QuotationId) -> () {
            let stringified = original.to_string();
            let reparsed = stringified.parse::<QuotationId>();

            assert_eq!(Ok(original), reparsed);
        }

        fn quotation_id_string_roundtrip_conv_2(s: String) -> TestResult {
            let parsed = match s.parse::<QuotationId>() {
                Ok(qid) => qid,
                Err(_) => return TestResult::discard(),
            };
            let restringified = parsed.to_string();
            let reparsed = restringified.parse::<QuotationId>();

            assert_eq!(Ok(parsed), reparsed);

            TestResult::passed()
        }

        fn chat_lines_stripped_preserves_qty_of_left_angle_brackets(
            text: String,
            id: QuotationId,
            file_id: QuotationFileId,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> () {
            let orig_left_angle_bracket_qty = text.matches('<').count();
            let quotation = Quotation {
                id,
                file_id,
                format: QuotationFormat::Chat,
                text,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
            };
            let left_angle_bracket_qty_after_trimming: usize = chat_lines_stripped(&quotation)
                .map(|s| s.matches('<').count())
                .sum();

            assert_eq!(
                left_angle_bracket_qty_after_trimming,
                orig_left_angle_bracket_qty
            );
        }

        fn quotation_byte_len_accuracy(
            text: String,
            id: QuotationId,
            file_id: QuotationFileId,
            format: QuotationFormat,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> TestResult {
            let quotation = Quotation {
                id,
                file_id,
                format,
                text,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
            };
            let arg = Default::default();
            let mut actual_len = 0;

            match for_each_quotation_text_piece(&arg, &quotation, &[], |s| actual_len += s.len()) {
                Ok(MustUse(_abridged)) => {}
                Err(_) => return TestResult::discard(),
            }

            assert_eq!(quotation_byte_len(&quotation), actual_len);

            TestResult::passed()
        }

        fn rendered_quotation_byte_len_bound_accuracy(
            text: String,
            id: QuotationId,
            file_id: QuotationFileId,
            format: QuotationFormat,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> TestResult {
            let quotation = Quotation {
                id,
                file_id,
                format,
                text,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
            };
            let rendered_text = match render_quotation(&Default::default(), &quotation, &[]) {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };
            let upper_bound = rendered_quotation_byte_len(&quotation);
            let actual_len = rendered_text.len();

            assert!(upper_bound >= actual_len);
            assert!(upper_bound <= actual_len + 1);

            TestResult::passed()
        }

        fn rendering_example_chat_1(
            id: QuotationId,
            file_id: QuotationFileId,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> TestResult {
            let text =
                "2018-03-24 09:31 <c74d> I do have a sense of humor. It just might not like \
                 yours.\n\
                 2018-03-24 09:31 <c74d> And yours might not like mine, and I don't think either \
                 of us should feel obliged to apologize for not liking the other's.\n\
                 2018-03-24 09:31 <c74d> However, I'm open to the idea that either or both of us \
                 should apologize for our own sense of humor.\n"
                    .into();

            let quotation = Quotation {
                id,
                file_id,
                format: QuotationFormat::Chat,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
                text,
            };

            let mut lines = chat_lines_stripped(&quotation);

            assert_eq!(
                lines.next(),
                Some(
                    "<c74d> I do have a sense of humor. It just might not like yours."
                )
            );
            assert_eq!(lines.next(), Some(
                "<c74d> And yours might not like mine, and I don't think either of us should feel \
                 obliged to apologize for not liking the other's."
            ));
            assert_eq!(lines.next(), Some(
                "<c74d> However, I'm open to the idea that either or both of us should apologize \
                 for our own sense of humor."
            ));
            assert_eq!(lines.next(), None);

            let rendered_text = match render_quotation(&Default::default(), &quotation, &[]) {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            assert_eq!(
                rendered_text,
                format!(
                    "[{id}] <c74d> I do have a sense of humor. It just might not like yours. \
                     <c74d> And yours might not like mine, and I don't think either of us should \
                     feel obliged to apologize for not liking the other's. \
                     <c74d> However, I'm open to the idea that either or both of us should \
                     apologize for our own sense of humor.",
                    id = quotation.id,
                )
            );

            TestResult::passed()
        }

        fn rendering_example_chat_2(
            id: QuotationId,
            file_id: QuotationFileId,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> TestResult {
            // Test different notations for `/me` messages, join messages, etc.
            let text =
                "2018-08-28 00:48 <foo> bar xyz\n\
                 2018-08-28 00:48  * foo summons quux\n\
                 2018-08-28 00:48 -!- quux has joined\n\
                 2018-08-28 00:48 -*- quux frobs foo\n\
                 2018-08-28 00:48 <foo> abc baz\n\
                 2018-08-28 00:48 <-- foo has left"
                    .into();

            let quotation = Quotation {
                id,
                file_id,
                format: QuotationFormat::Chat,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
                text,
            };

            let mut lines = chat_lines_stripped(&quotation);

            assert_eq!(lines.next(), Some("<foo> bar xyz"));
            assert_eq!(lines.next(), Some("* foo summons quux"));
            assert_eq!(lines.next(), Some("-*- quux frobs foo"));
            assert_eq!(lines.next(), Some("<foo> abc baz"));
            assert_eq!(lines.next(), Some("<-- foo has left"));
            assert_eq!(lines.next(), None);

            let rendered_text = match render_quotation(&Default::default(), &quotation, &[]) {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            assert_eq!(
                rendered_text,
                format!(
                    "{{{id}}} <foo> bar xyz * foo summons quux -*- quux frobs foo <foo> abc baz \
                     <-- foo has left",
                    id = quotation.id,
                )
            );

            TestResult::passed()
        }

        fn rendering_example_plain_1(
            id: QuotationId,
            file_id: QuotationFileId,
            tags: Vec<String>,
            anti_ping_tactic: AntiPingTactic
        ) -> TestResult {
            let text =
                "“I recognize that I am only making an assertion and furnishing no proof; I am \
                 sorry, but this is a habit of mine; sorry also that I am not alone in it; \
                 everybody seems to have this disease.” — Mark Twain"
                    .into();

            let quotation = Quotation {
                id,
                file_id,
                format: QuotationFormat::Plain,
                tags: tags.into_iter().map(Into::into).collect(),
                url: Default::default(),
                anti_ping_tactic,
                text,
            };

            let rendered_text = match render_quotation(&Default::default(), &quotation, &[]) {
                Ok(s) => s,
                Err(_) => return TestResult::discard(),
            };

            assert_eq!(
                rendered_text,
                format!(
                    "[{id}] “I recognize that I am only making an assertion and furnishing no \
                     proof; I am sorry, but this is a habit of mine; sorry also that I am not \
                     alone in it; everybody seems to have this disease.” — Mark Twain",
                    id = quotation.id,
                )
            );

            TestResult::passed()
        }
    }
}
