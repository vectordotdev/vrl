//! Contains the main "Value" type for Vector and VRL, as well as helper methods.

#[allow(clippy::module_name_repetitions)]
pub use super::value::regex::ValueRegex;
#[allow(clippy::module_name_repetitions)]
pub use iter::{IterItem, ValueIter};

use bytes::{Bytes, BytesMut};
use bytestring::ByteString;
use chrono::{DateTime, SecondsFormat, Utc};
use ordered_float::NotNan;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::{cmp::Ordering, collections::BTreeMap};

use super::KeyString;
use crate::path::ValuePath;

mod convert;
mod crud;
mod display;
mod iter;
mod path;
mod regex;

#[cfg(any(test, feature = "arbitrary"))]
mod arbitrary;
#[cfg(any(test, feature = "lua"))]
mod lua;
mod serde;

/// A boxed `std::error::Error`.
pub type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// The storage mapping for the `Object` variant.
pub type ObjectMap = BTreeMap<KeyString, Value>;

/// The main value type used in Vector events, and VRL.
#[derive(Debug, Clone)]
pub enum Value {
    /// Bytes - usually representing a UTF8 String.
    Bytes(Bytes),

    /// Guaranteed-UTF-8 string.
    String(ByteString),

    /// Regex.
    /// When used in the context of Vector this is treated identically to Bytes. It has
    /// additional meaning in the context of VRL.
    Regex(ValueRegex),

    /// Integer.
    Integer(i64),

    /// Float - not NaN.
    Float(NotNan<f64>),

    /// Boolean.
    Boolean(bool),

    /// Timestamp (UTC).
    Timestamp(DateTime<Utc>),

    /// Object.
    Object(ObjectMap),

    /// Array.
    Array(Vec<Value>),

    /// Null.
    Null,
}

impl Value {
    pub(crate) fn concat_bytes(lhs: &[u8], rhs: &[u8]) -> Bytes {
        #[allow(clippy::arithmetic_side_effects)]
        let mut value = BytesMut::with_capacity(lhs.len() + rhs.len());
        value.extend_from_slice(lhs);
        value.extend_from_slice(rhs);
        value.freeze()
    }

    pub(crate) fn concat_strings(lhs: &ByteString, rhs: &ByteString) -> ByteString {
        let bytes = Self::concat_bytes(lhs.as_bytes(), rhs.as_bytes());
        // SAFETY: concatenating two UTF-8 strings is UTF-8.
        unsafe { ByteString::from_bytes_unchecked(bytes) }
    }

    pub(crate) fn concat_bytes_with_string(lhs: &Bytes, rhs: &ByteString) -> Bytes {
        Self::concat_bytes(lhs, rhs.as_bytes())
    }

    pub(crate) fn concat_string_with_bytes(lhs: &ByteString, rhs: &Bytes) -> Bytes {
        Self::concat_bytes(lhs.as_bytes(), rhs)
    }

    /// Returns a string description of the value type
    pub const fn kind_str(&self) -> &str {
        match self {
            Self::Bytes(_) | Self::String(_) | Self::Regex(_) => "string",
            Self::Timestamp(_) => "timestamp",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::Boolean(_) => "boolean",
            Self::Object(_) => "map",
            Self::Array(_) => "array",
            Self::Null => "null",
        }
    }

    /// Merges `incoming` value into self.
    ///
    /// Will concatenate `Bytes`/`String` and overwrite the rest value kinds.
    pub fn merge(&mut self, incoming: Self) {
        match (self, incoming) {
            (Self::String(dst), Self::String(src)) => {
                *dst = Self::concat_strings(dst, &src);
            }
            (Self::Bytes(dst), Self::Bytes(src)) => {
                *dst = Self::concat_bytes(dst, &src);
            }
            (Self::Bytes(dst), Self::String(src)) => {
                *dst = Self::concat_bytes_with_string(dst, &src);
            }
            (current @ Self::String(_), Self::Bytes(src)) => {
                *current = Self::Bytes(Self::concat_string_with_bytes(
                    match current {
                        Self::String(dst) => dst,
                        _ => unreachable!("matched String"),
                    },
                    &src,
                ));
            }
            (current, incoming) => *current = incoming,
        }
    }

    /// Return if the node is empty, that is, it is an array or map with no items.
    ///
    /// ```rust
    /// use vrl::value::Value;
    /// use std::collections::BTreeMap;
    /// use vrl::path;
    ///
    /// let val = Value::from(1);
    /// assert_eq!(val.is_empty(), false);
    ///
    /// let mut val = Value::from(Vec::<Value>::default());
    /// assert_eq!(val.is_empty(), true);
    /// val.insert(path!(0), 1);
    /// assert_eq!(val.is_empty(), false);
    /// val.insert(path!(3), 1);
    /// assert_eq!(val.is_empty(), false);
    ///
    /// let mut val = Value::from(BTreeMap::default());
    /// assert_eq!(val.is_empty(), true);
    /// val.insert(path!("foo"), 1);
    /// assert_eq!(val.is_empty(), false);
    /// val.insert(path!("bar"), 2);
    /// assert_eq!(val.is_empty(), false);
    /// ```
    pub fn is_empty(&self) -> bool {
        match &self {
            Self::Boolean(_)
            | Self::Bytes(_)
            | Self::String(_)
            | Self::Regex(_)
            | Self::Timestamp(_)
            | Self::Float(_)
            | Self::Integer(_) => false,
            Self::Null => true,
            Self::Object(v) => v.is_empty(),
            Self::Array(v) => v.is_empty(),
        }
    }

    /// Returns a reference to a field value specified by a path iter.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert<'a>(
        &mut self,
        path: impl ValuePath<'a>,
        insert_value: impl Into<Self>,
    ) -> Option<Self> {
        let insert_value = insert_value.into();
        let path_iter = path.segment_iter().peekable();

        crud::insert(self, (), path_iter, insert_value)
    }

    /// Removes field value specified by the given path and return its value.
    ///
    /// A special case worth mentioning: if there is a nested array and an item is removed
    /// from the middle of this array, then it is just replaced by `Value::Null`.
    #[allow(clippy::needless_pass_by_value)]
    pub fn remove<'a>(&mut self, path: impl ValuePath<'a>, prune: bool) -> Option<Self> {
        crud::remove(self, &(), path.segment_iter(), prune)
            .map(|(prev_value, _is_empty)| prev_value)
    }

    /// Returns a reference to a field value specified by a path iter.
    #[allow(clippy::needless_pass_by_value)]
    pub fn get<'a>(&self, path: impl ValuePath<'a>) -> Option<&Self> {
        crud::get(self, path.segment_iter())
    }

    /// Get a mutable borrow of the value by path
    #[allow(clippy::needless_pass_by_value)]
    pub fn get_mut<'a>(&mut self, path: impl ValuePath<'a>) -> Option<&mut Self> {
        crud::get_mut(self, path.segment_iter())
    }

    /// Determine if the lookup is contained within the value.
    pub fn contains<'a>(&self, path: impl ValuePath<'a>) -> bool {
        self.get(path).is_some()
    }

    fn byte_content(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes.as_ref()),
            Self::String(s) => Some(s.as_bytes().as_ref()),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::String(a), Self::String(b)) => a.as_bytes() == b.as_bytes(),
            (Self::Bytes(a), Self::String(b)) => a == b.as_bytes(),
            (Self::String(a), Self::Bytes(b)) => a.as_bytes() == b,
            (Self::Regex(a), Self::Regex(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Boolean(a), Self::Boolean(b)) => a == b,
            (Self::Timestamp(a), Self::Timestamp(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Null, Self::Null) => true,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Bytes(v) => {
                0_u8.hash(state);
                v.as_ref().hash(state);
            }
            Self::String(v) => {
                0_u8.hash(state);
                v.as_bytes().as_ref().hash(state);
            }
            Self::Regex(v) => {
                1_u8.hash(state);
                v.hash(state);
            }
            Self::Integer(v) => {
                2_u8.hash(state);
                v.hash(state);
            }
            Self::Float(v) => {
                3_u8.hash(state);
                v.hash(state);
            }
            Self::Boolean(v) => {
                4_u8.hash(state);
                v.hash(state);
            }
            Self::Timestamp(v) => {
                5_u8.hash(state);
                v.hash(state);
            }
            Self::Object(v) => {
                6_u8.hash(state);
                v.hash(state);
            }
            Self::Array(v) => {
                7_u8.hash(state);
                v.hash(state);
            }
            Self::Null => {
                8_u8.hash(state);
            }
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if let (Some(a), Some(b)) = (self.byte_content(), other.byte_content()) {
            return a.partial_cmp(b);
        }
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return None;
        }
        match (self, other) {
            (Self::Bytes(_) | Self::String(_), _) => unreachable!("handled by byte content"),
            (Self::Regex(a), Self::Regex(b)) => a.partial_cmp(b),
            (Self::Integer(a), Self::Integer(b)) => a.partial_cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Boolean(a), Self::Boolean(b)) => a.partial_cmp(b),
            (Self::Timestamp(a), Self::Timestamp(b)) => a.partial_cmp(b),
            (Self::Object(a), Self::Object(b)) => a.partial_cmp(b),
            (Self::Array(a), Self::Array(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

/// Converts a slice of bytes to a string, including invalid characters.
#[must_use]
pub fn simdutf_bytes_utf8_lossy(v: &[u8]) -> Cow<'_, str> {
    simdutf8::basic::from_utf8(v).map_or_else(
        |_| {
            const REPLACEMENT: &str = "\u{FFFD}";

            let mut res = String::with_capacity(v.len());
            for chunk in v.utf8_chunks() {
                res.push_str(chunk.valid());
                if !chunk.invalid().is_empty() {
                    res.push_str(REPLACEMENT);
                }
            }
            Cow::Owned(res)
        },
        Cow::Borrowed,
    )
}

/// Converts a timestamp to a `String`.
#[must_use]
pub fn timestamp_to_string(timestamp: &DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

#[cfg(test)]
mod test {
    use quickcheck::{QuickCheck, TestResult};

    use crate::path;
    use crate::path::{BorrowedSegment, parse_value_path};

    use super::*;

    mod corner_cases {
        use super::*;

        #[test]
        fn remove_prune_map_with_map() {
            let mut value = Value::from(BTreeMap::default());
            let key = parse_value_path("foo.bar").unwrap();
            let marker = Value::from(true);
            assert_eq!(value.insert(&key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(&key, true), Some(marker));
            assert!(!value.contains(path!("foo")));
        }

        #[test]
        fn remove_prune_map_with_array() {
            let mut value = Value::from(BTreeMap::default());
            let key = parse_value_path("foo[0]").unwrap();
            let marker = Value::from(true);
            assert_eq!(value.insert(&key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(&key, true), Some(marker));
            assert!(!value.contains(path!("foo")));
        }

        #[test]
        fn remove_prune_array_with_map() {
            let mut value = Value::from(Vec::<Value>::default());
            let key = parse_value_path("[0].bar").unwrap();
            let marker = Value::from(true);
            assert_eq!(value.insert(&key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(&key, true), Some(marker));
            assert!(!value.contains(path!(0)));
        }

        #[test]
        fn remove_prune_array_with_array() {
            let mut value = Value::from(Vec::<Value>::default());
            let key = parse_value_path("[0][0]").unwrap();
            let marker = Value::from(true);
            assert_eq!(value.insert(&key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(&key, true), Some(marker));
            assert!(!value.contains(path!(0)));
        }
    }

    #[test]
    fn quickcheck_value() {
        fn inner(mut path: Vec<BorrowedSegment<'static>>) -> TestResult {
            let mut value = Value::from(BTreeMap::default());
            let mut marker = Value::from(true);

            // Push a field at the start of the path so the top level is a map.
            path.insert(0, BorrowedSegment::from("field"));

            assert_eq!(value.insert(&path, marker.clone()), None, "inserting value");
            assert_eq!(value.get(&path), Some(&marker), "retrieving value");
            assert_eq!(
                value.get_mut(&path),
                Some(&mut marker),
                "retrieving mutable value"
            );

            assert_eq!(value.remove(&path, true), Some(marker), "removing value");

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .max_tests(200)
            .quickcheck(inner as fn(Vec<BorrowedSegment<'static>>) -> TestResult);
    }

    #[test]
    fn partial_ord_value() {
        assert_eq!(
            Value::from(50).partial_cmp(&Value::from(77)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::from("zzz").partial_cmp(&Value::from("aaa")),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Value::from(10.5).partial_cmp(&Value::from(10.5)),
            Some(Ordering::Equal)
        );
        assert_eq!(Value::from(10.5).partial_cmp(&Value::from(10)), None);
    }

    fn hash_of(value: &Value) -> u64 {
        use std::hash::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn bytes_and_string_are_equal_and_hash_equal() {
        let string = Value::from("foo");
        let bytes = Value::from_static_bytes("foo");

        assert!(matches!(string, Value::String(_)));
        assert!(matches!(bytes, Value::Bytes(_)));
        assert_eq!(string, bytes);
        assert_eq!(hash_of(&string), hash_of(&bytes));
    }

    #[test]
    fn bytes_and_string_order_by_content() {
        assert_eq!(
            Value::from("aaa").partial_cmp(&Value::from_static_bytes("zzz")),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::from_static_bytes("zzz").partial_cmp(&Value::from("aaa")),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Value::from("foo").partial_cmp(&Value::from_static_bytes("foo")),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn merge_concat_preserves_string_only_when_both_are_string() {
        let mut both = Value::from("foo");
        both.merge(Value::from("bar"));
        assert!(matches!(both, Value::String(_)));
        assert_eq!(both, Value::from("foobar"));

        let mut mixed = Value::from("foo");
        mixed.merge(Value::from_static_bytes("bar"));
        assert!(matches!(mixed, Value::Bytes(_)));
        assert_eq!(mixed, Value::from("foobar"));
    }
}
