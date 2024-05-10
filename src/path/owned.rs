use std::fmt::{self, Debug, Display, Formatter, Write};
use std::str::FromStr;

use once_cell::sync::Lazy;
#[cfg(any(test, feature = "proptest"))]
use proptest::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::PathPrefix;
use super::{parse_target_path, parse_value_path, BorrowedSegment, PathParseError, ValuePath};
use crate::value::KeyString;

/// A lookup path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OwnedValuePath {
    pub segments: Vec<OwnedSegment>,
}

impl OwnedValuePath {
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn root() -> Self {
        vec![].into()
    }

    pub fn push_field(&mut self, field: &str) {
        self.segments.push(OwnedSegment::field(field));
    }

    pub fn push_segment(&mut self, segment: OwnedSegment) {
        self.segments.push(segment);
    }

    pub fn push_front_field(&mut self, field: &str) {
        self.segments.insert(0, OwnedSegment::field(field));
    }

    pub fn push_front_segment(&mut self, segment: OwnedSegment) {
        self.segments.insert(0, segment);
    }

    pub fn with_field_appended(&self, field: &str) -> Self {
        let mut new_path = self.clone();
        new_path.push_field(field);
        new_path
    }

    pub fn with_field_prefix(&self, field: &str) -> Self {
        self.with_segment_prefix(OwnedSegment::field(field))
    }

    pub fn with_segment_prefix(&self, segment: OwnedSegment) -> Self {
        let mut new_path = self.clone();
        new_path.push_front_segment(segment);
        new_path
    }

    pub fn push_index(&mut self, index: isize) {
        self.segments.push(OwnedSegment::index(index));
    }

    pub fn with_index_appended(&self, index: isize) -> Self {
        let mut new_path = self.clone();
        new_path.push_index(index);
        new_path
    }

    pub fn single_field(field: &str) -> Self {
        vec![OwnedSegment::field(field)].into()
    }

    /// Create the possible fields that can be followed by this lookup.
    ///
    /// The limit specifies the limit of the path depth we are interested in.
    /// Metrics is only interested in fields that are up to 3 levels deep (2 levels + 1 to check it
    /// terminates).
    ///
    /// eg, .tags.nork.noog will never be an accepted path so we don't need to spend the time
    /// collecting it.
    pub fn to_alternative_components(&self, limit: usize) -> Vec<Vec<&str>> {
        let mut components = vec![vec![]];
        for segment in self.segments.iter().take(limit) {
            match segment {
                OwnedSegment::Field(field) => {
                    for component in &mut components {
                        component.push(field.as_str());
                    }
                }

                OwnedSegment::Index(_) => {
                    return Vec::new();
                }
            }
        }

        components
    }

    pub fn push(&mut self, segment: OwnedSegment) {
        self.segments.push(segment);
    }
}

// OwnedValuePath values must have at least one segment.
#[cfg(any(test, feature = "proptest"))]
impl Arbitrary for OwnedValuePath {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        prop::collection::vec(any::<OwnedSegment>(), 1..10)
            .prop_map(|segments| OwnedValuePath { segments })
            .boxed()
    }
}

/// An owned path that contains a target (pointing to either an Event or Metadata)
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(try_from = "String", into = "String")]
pub struct OwnedTargetPath {
    pub prefix: PathPrefix,
    pub path: OwnedValuePath,
}

impl OwnedTargetPath {
    pub fn event_root() -> Self {
        Self::root(PathPrefix::Event)
    }
    pub fn metadata_root() -> Self {
        Self::root(PathPrefix::Metadata)
    }

    pub fn root(prefix: PathPrefix) -> Self {
        Self {
            prefix,
            path: OwnedValuePath::root(),
        }
    }

    pub fn event(path: OwnedValuePath) -> Self {
        Self {
            prefix: PathPrefix::Event,
            path,
        }
    }

    pub fn metadata(path: OwnedValuePath) -> Self {
        Self {
            prefix: PathPrefix::Metadata,
            path,
        }
    }

    pub fn can_start_with(&self, prefix: &Self) -> bool {
        if self.prefix != prefix.prefix {
            return false;
        }
        (&self.path).can_start_with(&prefix.path)
    }

    pub fn with_field_appended(&self, field: &str) -> Self {
        let mut new_path = self.path.clone();
        new_path.push_field(field);
        Self {
            prefix: self.prefix,
            path: new_path,
        }
    }

    pub fn with_index_appended(&self, index: isize) -> Self {
        let mut new_path = self.path.clone();
        new_path.push_index(index);
        Self {
            prefix: self.prefix,
            path: new_path,
        }
    }
}

impl Display for OwnedTargetPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.prefix {
            PathPrefix::Event => write!(f, ".")?,
            PathPrefix::Metadata => write!(f, "%")?,
        }
        Display::fmt(&self.path, f)
    }
}

impl Debug for OwnedTargetPath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<OwnedTargetPath> for String {
    fn from(target_path: OwnedTargetPath) -> Self {
        Self::from(&target_path)
    }
}

impl From<&OwnedTargetPath> for String {
    fn from(target_path: &OwnedTargetPath) -> Self {
        target_path.to_string()
    }
}

impl Display for OwnedValuePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from(self))
    }
}

impl FromStr for OwnedValuePath {
    type Err = PathParseError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        parse_value_path(src).map_err(|_| PathParseError::InvalidPathSyntax {
            path: src.to_owned(),
        })
    }
}

impl TryFrom<String> for OwnedValuePath {
    type Error = PathParseError;

    fn try_from(src: String) -> Result<Self, Self::Error> {
        src.parse()
    }
}

impl FromStr for OwnedTargetPath {
    type Err = PathParseError;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        parse_target_path(src).map_err(|_| PathParseError::InvalidPathSyntax {
            path: src.to_owned(),
        })
    }
}

impl TryFrom<String> for OwnedTargetPath {
    type Error = PathParseError;

    fn try_from(src: String) -> Result<Self, Self::Error> {
        src.parse()
    }
}

impl TryFrom<KeyString> for OwnedValuePath {
    type Error = PathParseError;

    fn try_from(src: KeyString) -> Result<Self, Self::Error> {
        src.parse()
    }
}

impl TryFrom<KeyString> for OwnedTargetPath {
    type Error = PathParseError;

    fn try_from(src: KeyString) -> Result<Self, Self::Error> {
        src.parse()
    }
}

impl From<OwnedValuePath> for String {
    fn from(owned: OwnedValuePath) -> Self {
        Self::from(&owned)
    }
}

impl From<&OwnedValuePath> for String {
    fn from(owned: &OwnedValuePath) -> Self {
        let mut output = String::new();
        for (i, segment) in owned.segments.iter().enumerate() {
            match segment {
                OwnedSegment::Field(field) => {
                    serialize_field(&mut output, field.as_ref(), (i != 0).then_some("."))
                }
                OwnedSegment::Index(index) => {
                    write!(output, "[{index}]").expect("Could not write to string")
                }
            }
        }
        output
    }
}

fn serialize_field(string: &mut String, field: &str, separator: Option<&str>) {
    // These characters should match the ones from the parser, implemented in `JitLookup`
    let needs_quotes = field.is_empty()
        || field
            .chars()
            .any(|c| !matches!(c, 'A'..='Z' | 'a'..='z' | '_' | '0'..='9' | '@'));

    // Reserve enough to fit the field, a `.` and two `"` characters. This
    // should suffice for the majority of cases when no escape sequence is used.
    let separator_len = separator.map_or(0, |x| x.len());
    string.reserve(field.as_bytes().len() + 2 + separator_len);
    if let Some(separator) = separator {
        string.push_str(separator);
    }
    if needs_quotes {
        string.push('"');
        for c in field.chars() {
            if matches!(c, '"' | '\\') {
                string.push('\\');
            }
            string.push(c);
        }
        string.push('"');
    } else {
        string.push_str(field);
    }
}

impl From<Vec<OwnedSegment>> for OwnedValuePath {
    fn from(segments: Vec<OwnedSegment>) -> Self {
        Self { segments }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
pub enum OwnedSegment {
    Field(KeyString),
    Index(isize),
}

impl OwnedSegment {
    pub fn field(value: &str) -> OwnedSegment {
        OwnedSegment::Field(value.into())
    }
    pub fn index(value: isize) -> OwnedSegment {
        OwnedSegment::Index(value)
    }

    pub fn is_field(&self) -> bool {
        matches!(self, OwnedSegment::Field(_))
    }
    pub fn is_index(&self) -> bool {
        matches!(self, OwnedSegment::Index(_))
    }

    pub fn can_start_with(&self, prefix: &OwnedSegment) -> bool {
        match (self, prefix) {
            (OwnedSegment::Index(a), OwnedSegment::Index(b)) => a == b,
            (OwnedSegment::Index(_), _) | (_, OwnedSegment::Index(_)) => false,
            (OwnedSegment::Field(a), OwnedSegment::Field(b)) => a == b,
        }
    }
}

impl<'a> From<&'a str> for OwnedSegment {
    fn from(field: &'a str) -> Self {
        OwnedSegment::field(field)
    }
}

impl<'a> From<&'a String> for OwnedSegment {
    fn from(field: &'a String) -> Self {
        OwnedSegment::field(field.as_str())
    }
}

impl From<isize> for OwnedSegment {
    fn from(index: isize) -> Self {
        OwnedSegment::index(index)
    }
}

impl<'a> ValuePath<'a> for &'a Vec<OwnedSegment> {
    type Iter = OwnedSegmentSliceIter<'a>;

    fn segment_iter(&self) -> Self::Iter {
        OwnedSegmentSliceIter(self.iter())
    }
}

impl<'a> ValuePath<'a> for &'a [OwnedSegment] {
    type Iter = OwnedSegmentSliceIter<'a>;

    fn segment_iter(&self) -> Self::Iter {
        OwnedSegmentSliceIter(self.iter())
    }
}

impl<'a> ValuePath<'a> for &'a OwnedValuePath {
    type Iter = OwnedSegmentSliceIter<'a>;

    fn segment_iter(&self) -> Self::Iter {
        (&self.segments).segment_iter()
    }
}

impl<'a> TryFrom<BorrowedSegment<'a>> for OwnedSegment {
    type Error = ();

    fn try_from(segment: BorrowedSegment<'a>) -> Result<Self, Self::Error> {
        match segment {
            BorrowedSegment::Invalid => Err(()),
            BorrowedSegment::Index(i) => Ok(OwnedSegment::Index(i)),
            BorrowedSegment::Field(field) => Ok(OwnedSegment::Field(field.into())),
        }
    }
}

static VALID_FIELD: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[0-9]*[a-zA-Z_@][0-9a-zA-Z_@]*$").unwrap());

fn format_field(f: &mut Formatter<'_>, field: &str) -> fmt::Result {
    // This can eventually just parse the field and see if it's valid, but the
    // parser is currently lenient in what it accepts so it doesn't catch all cases that
    // should be quoted
    let needs_quotes = !VALID_FIELD.is_match(field);
    if needs_quotes {
        write!(f, "\"{field}\"")
    } else {
        write!(f, "{field}")
    }
}

impl Display for OwnedSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OwnedSegment::Index(i) => write!(f, "[{i}]"),
            OwnedSegment::Field(field) => format_field(f, field),
        }
    }
}

#[derive(Clone)]
pub struct OwnedSegmentSliceIter<'a>(std::slice::Iter<'a, OwnedSegment>);

impl<'a> Iterator for OwnedSegmentSliceIter<'a> {
    type Item = BorrowedSegment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(BorrowedSegment::from)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::path::parse_value_path;

    #[test]
    fn owned_path_serialize() {
        let test_cases = [
            (".", Some("")),
            ("", None),
            ("]", None),
            ("]foo", None),
            ("..", None),
            ("...", None),
            ("f", Some("f")),
            ("foo", Some("foo")),
            (
                r#"ec2.metadata."availability-zone""#,
                Some(r#"ec2.metadata."availability-zone""#),
            ),
            ("@timestamp", Some("@timestamp")),
            ("foo[", None),
            ("foo$", None),
            (r#""$peci@l chars""#, Some(r#""$peci@l chars""#)),
            ("foo.foo bar", None),
            (r#"foo."foo bar".bar"#, Some(r#"foo."foo bar".bar"#)),
            ("[1]", Some("[1]")),
            ("[42]", Some("[42]")),
            ("foo.[42]", None),
            ("[42].foo", Some("[42].foo")),
            ("[-1]", Some("[-1]")),
            ("[-42]", Some("[-42]")),
            ("[-42].foo", Some("[-42].foo")),
            ("[-42]foo", Some("[-42].foo")),
            (r#""[42]. {}-_""#, Some(r#""[42]. {}-_""#)),
            (r#""a\"a""#, Some(r#""a\"a""#)),
            (r#"foo."a\"a"."b\\b".bar"#, Some(r#"foo."a\"a"."b\\b".bar"#)),
            ("<invalid>", None),
            (r#""🤖""#, Some(r#""🤖""#)),
        ];

        for (path, expected) in test_cases {
            let path = parse_value_path(path).map(String::from).ok();

            assert_eq!(path, expected.map(|x| x.to_owned()));
        }
    }

    fn reparse_thing<T: std::fmt::Debug + std::fmt::Display + Eq + FromStr>(thing: T)
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        let text = thing.to_string();
        let thing2: T = text.parse().unwrap();
        assert_eq!(thing, thing2);
    }

    proptest::proptest! {
        #[test]
        fn reparses_valid_value_path(path: OwnedValuePath) {
            reparse_thing(path);
        }

        #[test]
        fn reparses_valid_target_path(path: OwnedTargetPath) {
            reparse_thing(path);
        }
    }
}
