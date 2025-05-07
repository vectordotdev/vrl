use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// The key type value. This is a simple zero-overhead wrapper set up to make it explicit that
/// object keys are read-only and their underlying type is opaque and may change for efficiency.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(any(test, feature = "proptest"), derive(proptest_derive::Arbitrary))]
#[serde(transparent)]
pub struct KeyString(String);

impl KeyString {
    /// Convert the key into a boxed slice of bytes (`u8`).
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.0.into_bytes().into()
    }

    /// Is this string empty?
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the length of the contained key.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return a reference to the contained string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for KeyString {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl AsRef<str> for KeyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for KeyString {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for KeyString {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for KeyString {
    fn eq(&self, that: &str) -> bool {
        self.0[..].eq(that)
    }
}

impl From<&str> for KeyString {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for KeyString {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<Cow<'_, str>> for KeyString {
    fn from(s: Cow<'_, str>) -> Self {
        Self(s.into())
    }
}

impl From<KeyString> for String {
    fn from(s: KeyString) -> Self {
        s.0
    }
}

#[cfg(any(test, feature = "arbitrary"))]
impl quickcheck::Arbitrary for KeyString {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        String::arbitrary(g).into()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let s = self.0.to_string();
        Box::new(s.shrink().map(Into::into))
    }
}

#[cfg(any(test, feature = "lua"))]
mod lua {
    use mlua::prelude::LuaResult;
    use mlua::{FromLua, IntoLua, Lua, Value as LuaValue};

    use super::KeyString;

    impl FromLua for KeyString {
        fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
            String::from_lua(value, lua).map(Self::from)
        }
    }

    impl IntoLua for KeyString {
        fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
            self.0.into_lua(lua)
        }
    }
}
