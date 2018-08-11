use smallvec;
use smallvec::SmallVec;
use std;
use std::borrow::Cow;
use yaml_rust;
use yaml_rust::yaml;
use yaml_rust::Yaml;

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
        TypeMismatch(path: String, expected_ty: Kind, actual_ty: Kind) {
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
    }
}

/// Predefined YAML string values.
pub mod str {
    use yaml_rust::Yaml;

    lazy_static! {
        pub static ref YAML_STR_CHAN: Yaml = Yaml::String("chan".into());
        pub static ref YAML_STR_CMD: Yaml = Yaml::String("cmd".into());
        pub static ref YAML_STR_LIST: Yaml = Yaml::String("list".into());
        pub static ref YAML_STR_MSG: Yaml = Yaml::String("msg".into());
        pub static ref YAML_STR_R: Yaml = Yaml::String("r".into());
        pub static ref YAML_STR_REGEX: Yaml = Yaml::String("regex".into());
        pub static ref YAML_STR_S: Yaml = Yaml::String("s".into());
        pub static ref YAML_STR_STRING: Yaml = Yaml::String("string".into());
    }
}

/// A predefined error message for use when one `expect`s that the framework will handle syntax
/// errors in command arguments for one.
pub static FW_SYNTAX_CHECK_FAIL: &str =
    "The framework should have caught this syntax error before it tried to run this command \
     handler!";

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
/// `lt_map` to construct a `Cow` with the desired lifetime (`lt_map` usually should be
/// `Cow::Borrowed`). If the `node` is not a `Yaml::String`, its `Debug` representation will be
/// returned, wrapped in `Cow::Owned`.
pub fn any_to_str<'a, 'b, F>(node: &'a Yaml, lt_map: F) -> Cow<'b, str>
where
    F: Fn(&'a str) -> Cow<'b, str>,
{
    node.as_str()
        .map(lt_map)
        .unwrap_or_else(|| Cow::Owned(format!("{:?}", node)))
}

/// Converts a scalar YAML node to a string.
///
/// If the `node` is scalar, returns the same value as `any_to_str`, except wrapped in
/// `Result::Ok`. If the `node` is a sequence, a mapping, or something stranger, returns an `Err`
/// containing a `Kind` value representing what particular kind of non-scalar `node` is.
pub fn scalar_to_str<'a, 'b, F>(
    node: &'a Yaml,
    lt_map: F,
) -> std::result::Result<Cow<'b, str>, Kind>
where
    F: Fn(&'a str) -> Cow<'b, str>,
{
    match Kind::of(node) {
        Kind::Scalar => Ok(any_to_str(node, lt_map)),
        kind => Err(kind),
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
            path_buf.join("."),
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
                any_to_str(key, Cow::Borrowed),
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
                    any_to_str(key, Cow::Borrowed),
                )?
            }
            (_, None) => bail!(ErrorKind::RequiredFieldMissing(any_to_str(
                key,
                |s| s.to_owned().into()
            ),)),
        }
    }

    Ok(())
}
