use ref_slice::ref_slice;
use smallvec;
use smallvec::SmallVec;
use std;
use std::borrow::Cow;
use std::iter;
use util::to_cow_owned;
use yaml_rust;
use yaml_rust::yaml;
use yaml_rust::Yaml;
use yaml_rust::YamlEmitter;

error_chain! {
    foreign_links {
        YamlEmit(yaml_rust::EmitError);
        YamlScan(yaml_rust::ScanError);
    }

    errors {
        NoSingleNode(node_qty: usize) {
            description("wanted a single YAML node but found zero or multiple nodes")
            display("While parsing YAML: Wanted a single node, but found {} nodes.", node_qty)
        }
        RequiredFieldMissing(name: Cow<'static, str>) {
            description("a YAML object is missing a required field")
            display("While handling YAML: An object is missing the required field {:?}.", name)
        }
        AliasesNotSupported {
            description("encountered a YAML alias (which is not supported by `yaml_rust`)")
            display("While handling YAML: Encountered a YAML alias, which is not supported by \
                     `yaml_rust`.")
        }
        TypeMismatch(path: Cow<'static, str>, expected_ty: Kind, actual_ty: Kind) {
            description("encountered a type error while handling YAML")
            display("While handling YAML: Expected {path} to be of type {expected_ty:?}, but it \
                     is of type {actual_ty:?}.",
                     path = path,
                     expected_ty = expected_ty,
                     actual_ty = actual_ty)
        }
        ExpectedNonEmptyStream {
            description("expected non-empty YAML stream but found empty stream")
            display("While handling YAML: Expected a non-empty stream, but found an empty stream.")
        }
        ExpectedEmptyStream {
            description("expected empty YAML stream but found non-empty stream")
            display("While handling YAML: Expected an empty stream, but found a non-empty stream.")
        }
        ArgGivenByBothLongAndShortKey(long_key: Cow<'static, str>, short_key: Cow<'static, str>) {
            description("wanted one but not both of an argument's full and abbreviated names")
            display("While handling YAML: An argument was given by both its full key {full:?} and \
                     its abbreviated key {abbr:?}; please use one or the other but not both.",
                    full = long_key,
                    abbr = short_key)
        }
    }
}

/// Predefined YAML string values.
pub mod str {
    use super::mk_str;
    use yaml_rust::Yaml;

    lazy_static! {
        pub static ref YAML_STR_CHAN: Yaml = mk_str("chan");
        pub static ref YAML_STR_CMD: Yaml = mk_str("cmd");
        pub static ref YAML_STR_ELLIPSIS: Yaml = mk_str("...");
        pub static ref YAML_STR_ELLIPSIS_IN_SQUARE_BRACKETS: Yaml = mk_str("[...]");
        pub static ref YAML_STR_ID: Yaml = mk_str("id");
        pub static ref YAML_STR_LIST: Yaml = mk_str("list");
        pub static ref YAML_STR_MSG: Yaml = mk_str("msg");
        pub static ref YAML_STR_R: Yaml = mk_str("r");
        pub static ref YAML_STR_REGEX: Yaml = mk_str("regex");
        pub static ref YAML_STR_S: Yaml = mk_str("s");
        pub static ref YAML_STR_STRING: Yaml = mk_str("string");
        pub static ref YAML_STR_TAG: Yaml = mk_str("tag");
    }
}

/// A predefined error message for use when one `expect`s that the framework will handle syntax
/// errors in command arguments for one.
pub static FW_SYNTAX_CHECK_FAIL: &str =
    "The framework should have caught this syntax error before it tried to run this command \
     handler!";

lazy_static! {
    /// An empty YAML mapping.
    pub static ref EMPTY_MAP: Yaml = mk_map(iter::empty());

    /// An empty YAML sequence.
    pub static ref EMPTY_SEQ: Yaml = mk_seq(iter::empty());

    /// An empty YAML string.
    pub static ref EMPTY_STR: Yaml = mk_str("");
}

#[derive(Copy, Clone, Debug)]
pub enum Kind {
    Scalar,
    Sequence,
    Mapping,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl Kind {
    pub fn of(node: &Yaml) -> Kind {
        Self::from_aug_ty(&AugmentedTy::of(node))
    }

    fn from_aug_ty(ty: &AugmentedTy) -> Kind {
        match ty {
            &AugmentedTy::Scalar => Kind::Scalar,
            &AugmentedTy::Sequence => Kind::Sequence,
            &AugmentedTy::Mapping(_) => Kind::Mapping,
            &AugmentedTy::Other => Kind::__Nonexhaustive,
        }
    }
}

#[derive(Debug)]
pub(crate) enum AugmentedTy<'a> {
    Scalar,
    Sequence,
    Mapping(&'a yaml::Hash),
    Other,
}

impl<'a> AugmentedTy<'a> {
    pub(crate) fn of(node: &Yaml) -> AugmentedTy {
        match node {
            &Yaml::Real(_)
            | &Yaml::Integer(_)
            | &Yaml::String(_)
            | &Yaml::Boolean(_)
            | &Yaml::Null => AugmentedTy::Scalar,
            &Yaml::Array(_) => AugmentedTy::Sequence,
            &Yaml::Hash(ref data) => AugmentedTy::Mapping(data),
            &Yaml::Alias(_) | &Yaml::BadValue => AugmentedTy::Other,
        }
    }
}

/// Converts any type of YAML node to a string.
///
/// If the `node` is a `Yaml::String`, a `&str` reference to its content it will be passed to
/// `lt_map` to construct a `Cow` with the desired lifetime. If the `node` is not a `Yaml::String`,
/// a stringified representation of it will be returned, wrapped in `Cow::Owned`. If such
/// stringification fails, an `Err` will be returned.
///
/// `lt_map` usually should be `Cow::Borrowed`; however, if `'b` is `'static` and `'a` is not,
/// `lt_map` usually should be `util::to_cow_owned`.
pub fn any_to_str<'a, 'b, F>(node: &'a Yaml, lt_map: F) -> Result<Cow<'b, str>>
where
    F: Fn(&'a str) -> Cow<'b, str>,
{
    match node.as_str() {
        Some(s) => Ok(lt_map(s)),
        None => {
            let mut s = String::new();

            {
                let mut emitter = YamlEmitter::new(&mut s);
                emitter.compact(true);
                emitter.dump(node)?;
            }

            Ok(to_cow_owned(s.trim_start_matches("---\n")))
        }
    }
}

/// Converts a scalar YAML node to a string.
///
/// If the `node` is scalar, returns the same value as `any_to_str(node, lt_map)`. If the `node` is
/// a sequence, a mapping, or something stranger, an `Err` containing a `TypeMismatch` error will
/// be returned.
///
/// The parameter `subject_label` serves to identify the `node` in any `TypeMismatch` error message
/// that may be generated.
pub fn scalar_to_str<'a, 'b, F, S1>(
    node: &'a Yaml,
    lt_map: F,
    subject_label: S1,
) -> Result<Cow<'b, str>>
where
    F: Fn(&'a str) -> Cow<'b, str>,
    S1: Into<Cow<'static, str>>,
{
    match Kind::of(node) {
        Kind::Scalar => any_to_str(node, lt_map),
        wrong_kind => {
            Err(ErrorKind::TypeMismatch(subject_label.into(), Kind::Scalar, wrong_kind).into())
        }
    }
}

/// Converts any type of YAML node to a sequence.
///
/// If the `node` is a sequence, a vector of references to its elements is returned. Otherwise, a
/// vector with `node` as its single element is returned.
///
/// Either a `&Yaml` or `Option<&Yaml>` can be passed as argument; in the latter case, `None` will
/// be treated as an empty sequence.
pub fn any_to_seq<'a, Y>(node: Y) -> SmallVec<[&'a Yaml; 8]>
where
    Y: Into<Option<&'a Yaml>>,
{
    match node.into() {
        Some(Yaml::Array(ref vec)) => vec.iter().collect(),
        Some(single) => SmallVec::from_slice(&[single]),
        None => SmallVec::new(),
    }
}

/// Returns an iterator over references to a YAML node's elements if the node is a sequence, or
/// over a sequence with the node as its single element otherwise.
///
/// Either a `&Yaml` or `Option<&Yaml>` can be passed as argument; in the latter case, `None` will
/// be treated as an empty sequence.
pub fn iter_as_seq<'a, Y>(node: Y) -> std::slice::Iter<'a, Yaml>
where
    Y: Into<Option<&'a Yaml>>,
{
    match node.into() {
        Some(Yaml::Array(ref vec)) => vec.iter(),
        Some(single) => ref_slice(single).iter(),
        None => [].iter(),
    }
}

/// Gets an argument from a hash-map of arguments by either an abbreviated ("short") form or the
/// full ("long") form of the argument's key (i.e., its "name").
///
/// If both the long and short forms of the argument's key are found, an `Err` is returned. If
/// neither is found, `Ok(None)` is returned.
pub fn get_arg_by_short_or_long_key<'a>(
    args: &'a yaml::Hash,
    short_key: &Yaml,
    long_key: &Yaml,
) -> Result<Option<&'a Yaml>> {
    match (args.get(short_key), args.get(long_key)) {
        (Some(v), None) | (None, Some(v)) => Ok(Some(v)),
        (Some(_), Some(_)) => Err(ErrorKind::ArgGivenByBothLongAndShortKey(
            any_to_str(long_key, to_cow_owned)?,
            any_to_str(short_key, to_cow_owned)?,
        ).into()),
        (None, None) => Ok(None),
    }
}

/// Parses a lone YAML node.
///
/// Wraps `yaml_rust::YamlLoader::load_from_str` to parse a single YAML node.
///
/// If this function parses a single YAML node `y`, it returns `Ok(Some(y))`. If given an empty
/// YAML stream, returns `Ok(None)`. If given a stream of multiple YAML documents, returns `Err`.
pub fn parse_node(src: &str) -> Result<Option<Yaml>> {
    let mut stream = yaml::YamlLoader::load_from_str(src)?;

    let node = stream.pop();

    match stream.len() {
        0 => Ok(node),
        n => {
            bail!(ErrorKind::NoSingleNode({
                // This addition should never overflow, because the stream length was previously
                // greater by one.
                n + 1
            }))
        }
    }
}

pub(crate) fn parse_and_check_node<'s, DefaultCtor, S1>(
    src: &str,
    expected_syntax: &'s Yaml,
    subject_label: S1,
    default: DefaultCtor,
) -> Result<Yaml>
where
    DefaultCtor: Fn() -> Yaml,
    S1: Into<Cow<'s, str>>,
{
    let node = parse_node(src)?.unwrap_or_else(default);

    check_type(expected_syntax, &node, subject_label)?;

    Ok(node)
}

/// Checks that a YAML object has a given type and structure.
///
/// Checks that the `actual` YAML object matches the type and structure of the `expected` YAML
/// object.
///
/// `subject_label` is a string that will identify the `actual` object in any error messages
/// produced.
pub(crate) fn check_type<'s, S1>(expected: &'s Yaml, actual: &Yaml, subject_label: S1) -> Result<()>
where
    S1: Into<Cow<'s, str>>,
{
    let subject_label = subject_label.into();

    let mut path_buf = SmallVec::<[_; 8]>::new();

    check_type_inner(expected, actual, &mut path_buf, subject_label)?;

    debug_assert!(path_buf.is_empty());

    Ok(())
}

fn check_type_inner<'s, AS>(
    expected: &'s Yaml,
    actual: &Yaml,
    path_buf: &mut SmallVec<AS>,
    subject_label: Cow<'s, str>,
) -> Result<()>
where
    AS: smallvec::Array<Item = Cow<'s, str>>,
{
    trace!(
        "Checking YAML object's type and structure. Expected: {expected:?}; actual: {actual:?}.",
        expected = expected,
        actual = actual
    );

    // If "..." or "[...]" is requested, it means that anything is acceptable.
    if [
        &*str::YAML_STR_ELLIPSIS,
        &*str::YAML_STR_ELLIPSIS_IN_SQUARE_BRACKETS,
    ]
        .contains(&expected)
    {
        return Ok(());
    }

    use util::yaml::AugmentedTy as Ty;

    path_buf.push(subject_label);

    let expected_ty = Ty::of(expected);
    let actual_ty = Ty::of(actual);

    match (&expected_ty, &actual_ty) {
        (&Ty::Scalar, &Ty::Scalar) | (&Ty::Sequence, &Ty::Sequence) => {
            // Types match trivially.
        }
        (&Ty::Mapping(expected_fields), &Ty::Mapping(actual_fields)) => {
            check_field_types(expected_fields, actual_fields, path_buf)?
        }
        (&Ty::Scalar, &Ty::Sequence)
        | (&Ty::Scalar, &Ty::Mapping(_))
        | (&Ty::Sequence, &Ty::Scalar)
        | (&Ty::Sequence, &Ty::Mapping(_))
        | (&Ty::Mapping(_), &Ty::Scalar)
        | (&Ty::Mapping(_), &Ty::Sequence) => bail!(ErrorKind::TypeMismatch(
            path_buf.join(".").into(),
            Kind::from_aug_ty(&expected_ty),
            Kind::from_aug_ty(&actual_ty),
        )),
        (_, &Ty::Other) | (&Ty::Other, _) => bail!(ErrorKind::AliasesNotSupported),
    }

    path_buf.pop();

    Ok(())
}

fn check_field_types<'s, AS>(
    expected_fields: &'s yaml::Hash,
    actual_fields: &yaml::Hash,
    path_buf: &mut SmallVec<AS>,
) -> Result<()>
where
    AS: smallvec::Array<Item = Cow<'s, str>>,
{
    for (key, expected_value) in expected_fields {
        match (expected_value, actual_fields.get(key)) {
            (_, Some(actual_value)) => check_type_inner(
                expected_value,
                actual_value,
                path_buf,
                any_to_str(key, Cow::Borrowed)?,
            )?,
            (&Yaml::String(ref s), None) if s.starts_with("[") && s.ends_with("]") => {
                // This field is optional.
            }
            (&Yaml::Array(_), None) => {
                // All sequence fields are treated as optional.
            }
            (&Yaml::Hash(_), None) => {
                // Treat an absent mapping as were it an empty mapping.
                check_type_inner(
                    expected_value,
                    &Yaml::Hash(Default::default()),
                    path_buf,
                    any_to_str(key, Cow::Borrowed)?,
                )?
            }
            (_, None) => bail!(ErrorKind::RequiredFieldMissing(any_to_str(key, |s| s
                .to_owned()
                .into())?)),
        }
    }

    Ok(())
}

#[inline]
pub fn mk_map<I>(entries: I) -> Yaml
where
    I: IntoIterator<Item = (Yaml, Yaml)>,
{
    Yaml::Hash(entries.into_iter().collect())
}

#[inline]
pub fn mk_seq<I>(entries: I) -> Yaml
where
    I: IntoIterator<Item = Yaml>,
{
    Yaml::Array(entries.into_iter().collect())
}

#[inline]
pub fn mk_str<S>(s: S) -> Yaml
where
    S: Into<String>,
{
    Yaml::String(s.into())
}

#[inline]
pub fn mk_true() -> Yaml {
    Yaml::Boolean(true)
}

#[inline]
pub fn mk_false() -> Yaml {
    Yaml::Boolean(false)
}

#[inline]
pub fn mk_int<N>(n: N) -> Yaml
where
    N: Into<i64>,
{
    Yaml::Integer(n.into())
}

#[cfg(test)]
mod tests {
    // NOTE: The parsing and type-checking functions are used only(?) by `core::bot_cmd`, and
    // they're tested via that module, in `core::bot_cmd::tests`.
}
