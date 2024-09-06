//! Contains a custom `Bytes` type used in the VRL `Value` which is optimized for access as a UTF-8
//! validated string. This implementation makes two assumptions:
//!
//! 1. Most instances of this type will be used as a UTF-8 validated string at some point in their
//! life, and most likely more than once.
//!
//! 2. Most instances of this type will be valid UTF-8 and so storing an extra copy of the
//! lossy-converted string is unnecessary overhead.

#![allow(missing_docs)]

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::{cmp::Ordering, string::FromUtf8Error};

/// The bytes storage type used in VRL's `Value` type.
#[derive(Clone, Debug, Eq)]
pub enum Bytes {
    /// Source data that is not valid UTF-8 are stored as `Bytes`.
    Invalid(bytes::Bytes),
    /// Source data that is valid UTF-8 are stored as a `String`.
    Valid(String),
}

impl From<&str> for Bytes {
    fn from(src: &str) -> Self {
        Self::Valid(src.into())
    }
}

impl From<String> for Bytes {
    fn from(src: String) -> Self {
        Self::Valid(src)
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(src: Vec<u8>) -> Self {
        String::from_utf8(src).map_or_else(
            |invalid: FromUtf8Error| Self::Invalid(invalid.into_bytes().into()),
            Self::Valid,
        )
    }
}

impl From<Box<[u8]>> for Bytes {
    fn from(src: Box<[u8]>) -> Self {
        Self::from(Vec::from(src))
    }
}

impl From<bytes::Bytes> for Bytes {
    fn from(src: bytes::Bytes) -> Self {
        Vec::from(src).into()
    }
}

// Comparisons and hashing

impl<'a, T: ?Sized> PartialEq<&'a T> for Bytes
where
    Self: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl PartialEq<Self> for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes_slice() == other.as_bytes_slice()
    }
}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes_slice() == other
    }
}

impl PartialEq<Bytes> for &[u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialEq<str> for Bytes {
    fn eq(&self, other: &str) -> bool {
        self.as_bytes_slice() == other.as_bytes()
    }
}

impl Hash for Bytes {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.as_bytes_slice().hash(hasher);
    }
}

impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_bytes_slice().cmp(other.as_bytes_slice())
    }
}

impl Bytes {
    pub fn len(&self) -> usize {
        match self {
            Self::Invalid(bytes) => bytes.len(),
            Self::Valid(string) => string.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Invalid(bytes) => bytes.is_empty(),
            Self::Valid(string) => string.is_empty(),
        }
    }

    #[must_use]
    pub fn copy_from_slice(bytes: &[u8]) -> Self {
        std::str::from_utf8(bytes).map_or_else(
            |_| Self::Invalid(bytes::Bytes::copy_from_slice(bytes)),
            |string| Self::Valid(string.into()),
        )
    }

    /// Returns a byte slice of the underying data.
    pub fn as_bytes_slice(&self) -> &[u8] {
        match self {
            Self::Invalid(bytes) => bytes.as_ref(),
            Self::Valid(string) => string.as_bytes(),
        }
    }

    /// Copy the underlying bytes into a `bytes::Bytes` object. This is unfortunately named, as the
    /// result is distinct from the above `as_bytes` method, but no better name was evident.
    pub fn to_bytes(&self) -> bytes::Bytes {
        match self {
            Self::Invalid(bytes) => bytes.clone(),
            Self::Valid(string) => bytes::Bytes::copy_from_slice(string.as_bytes()),
        }
    }

    /// Interpret the bytes as valid UTF-8 string, returning `None` if there are invalid bytes.
    pub fn as_utf8(&self) -> Option<&str> {
        match self {
            Self::Invalid(..) => None,
            Self::Valid(string) => Some(string.as_ref()),
        }
    }

    /// Copy the bytes into a UTF-8 string including invalid bytes.
    pub fn as_utf8_lossy(&self) -> Cow<'_, str> {
        match self {
            Self::Invalid(bytes) => String::from_utf8_lossy(bytes.as_ref()),
            Self::Valid(string) => Cow::Borrowed(string.as_ref()),
        }
    }

    /// Convert the owned bytes into a UTF-8 string including invalid bytes.
    #[must_use]
    pub fn to_utf8_lossy(&self) -> String {
        match self {
            Self::Invalid(bytes) => String::from_utf8_lossy(bytes.as_ref()).into_owned(),
            Self::Valid(string) => string.clone(),
        }
    }

    #[must_use]
    pub fn into_utf8_lossy(self) -> String {
        match self {
            Self::Invalid(bytes) => String::from_utf8_lossy(bytes.as_ref()).into_owned(),
            Self::Valid(string) => string,
        }
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        match self {
            Self::Invalid(bytes) => bytes.to_vec(),
            Self::Valid(string) => string.into_bytes(),
        }
    }

    /// In the `bytes::Bytes` type, this is implemented automatically by virtue of it implementing
    /// `Deref<Target = u8>`, but we specifically want to avoid implementing either `Deref` or
    /// `AsRef` in order to force the use of our accessors for efficiency.
    pub fn repeat(&self, count: usize) -> Vec<u8> {
        match self {
            Self::Invalid(bytes) => bytes.repeat(count),
            Self::Valid(string) => string.as_bytes().repeat(count),
        }
    }

    #[must_use]
    pub fn from_static(src: &'static [u8]) -> Self {
        std::str::from_utf8(src).map_or_else(
            |_| Self::Invalid(bytes::Bytes::from_static(src)),
            |s| Self::Valid(s.into()),
        )
    }

    pub fn chunks(&self, size: usize) -> std::slice::Chunks<'_, u8> {
        match self {
            Self::Invalid(bytes) => bytes.chunks(size),
            Self::Valid(string) => string.as_bytes().chunks(size),
        }
    }
}
