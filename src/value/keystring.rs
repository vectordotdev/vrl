use std::borrow::Cow;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// The key type for event objects. Backed by [`SmolStr`] so that strings up to
/// 22 bytes are stored inline (no heap allocation). Every capture-group name in
/// a typical regex (e.g. "host", "user", "timestamp") fits inline, making
/// `clone()` a plain 24-byte stack copy — no malloc, no atomic ops.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct KeyString(SmolStr);

impl KeyString {
    /// Convert the key into a boxed slice of bytes (`u8`).
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.0.as_bytes().to_vec().into_boxed_slice()
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
        self.0.as_str()
    }
}

impl Display for KeyString {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl AsRef<str> for KeyString {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::ops::Deref for KeyString {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}

impl std::borrow::Borrow<str> for KeyString {
    fn borrow(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq<str> for KeyString {
    fn eq(&self, that: &str) -> bool {
        self.0.as_str() == that
    }
}

impl From<&str> for KeyString {
    fn from(s: &str) -> Self {
        Self(SmolStr::new(s))
    }
}

impl From<String> for KeyString {
    fn from(s: String) -> Self {
        Self(SmolStr::new(s.as_str()))
    }
}

impl From<Cow<'_, str>> for KeyString {
    fn from(s: Cow<'_, str>) -> Self {
        Self(SmolStr::new(s.as_ref()))
    }
}

impl From<KeyString> for String {
    fn from(s: KeyString) -> Self {
        s.0.as_str().to_owned()
    }
}

#[cfg(any(test, feature = "arbitrary"))]
impl quickcheck::Arbitrary for KeyString {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        String::arbitrary(g).into()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        let s = self.0.as_str().to_string();
        Box::new(s.shrink().map(Into::into))
    }
}

#[cfg(any(test, feature = "proptest"))]
impl proptest::arbitrary::Arbitrary for KeyString {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;
        proptest::arbitrary::any::<String>()
            .prop_map(KeyString::from)
            .boxed()
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
            self.0.as_str().to_owned().into_lua(lua)
        }
    }
}
