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
use std::{cmp::Ordering, ops::RangeBounds, str::Utf8Error};

/// The bytes storage type used in VRL's `Value` type.
#[derive(Clone, Debug, Eq, Hash)]
pub struct Bytes(bytes::Bytes);

impl<T> From<T> for Bytes
where
    bytes::Bytes: From<T>,
{
    fn from(src: T) -> Self {
        Self(src.into())
    }
}

impl PartialEq<Bytes> for bytes::Bytes {
    fn eq(&self, other: &Bytes) -> bool {
        self.eq(&other.0)
    }
}

impl<T: ?Sized> PartialEq<T> for Bytes
where
    bytes::Bytes: PartialEq<T>,
{
    fn eq(&self, other: &T) -> bool {
        self.0.eq(other)
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        self.eq(&other.0)
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd<Bytes> for bytes::Bytes {
    fn partial_cmp(&self, other: &Bytes) -> Option<Ordering> {
        self.partial_cmp(&other.0)
    }
}

impl<T> PartialOrd<T> for Bytes
where
    bytes::Bytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &T) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Bytes {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn copy_from_slice(bytes: &[u8]) -> Self {
        Self(bytes::Bytes::copy_from_slice(bytes))
    }

    /// Returns a byte slice of the underying data.
    pub fn as_bytes_slice(&self) -> &[u8] {
        self.0.as_ref()
    }

    /// Copy the underlying bytes into a `bytes::Bytes` object.
    pub fn to_bytes(&self) -> bytes::Bytes {
        self.0.clone()
    }

    /// Interpret the bytes as valid UTF-8 string, returning `None` if there are invalid bytes.
    ///
    /// # Errors
    ///
    /// The same as `std::str::from_utf8`.
    pub fn as_utf8(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.0.as_ref())
    }

    /// Interpret the bytes as a UTF-8 string including invalid bytes.
    pub fn as_utf8_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.0.as_ref())
    }

    /// Copy the bytes into a UTF-8 string including invalid bytes.
    #[must_use]
    pub fn to_utf8_lossy(&self) -> String {
        self.as_utf8_lossy().into_owned()
    }

    /// Convert the owned bytes into a UTF-8 string including invalid bytes.
    #[must_use]
    pub fn into_utf8_lossy(self) -> String {
        self.as_utf8_lossy().into_owned()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// In the `bytes::Bytes` type, this is implemented automatically by virtue of it implementing
    /// `Deref<Target = u8>`, but we specifically want to avoid implementing either `Deref` or
    /// `AsRef` in order to force the use of our accessors for efficiency.
    pub fn repeat(&self, count: usize) -> Vec<u8> {
        self.0.repeat(count)
    }

    pub fn slice(&self, range: impl RangeBounds<usize>) -> bytes::Bytes {
        self.0.slice(range)
    }

    #[must_use]
    pub fn from_static(src: &'static [u8]) -> Self {
        Self(bytes::Bytes::from_static(src))
    }

    pub fn chunks(&self, size: usize) -> std::slice::Chunks<'_, u8> {
        self.0.chunks(size)
    }
}
