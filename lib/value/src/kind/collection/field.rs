use crate::kind::collection::{CollectionKey, CollectionRemove};
use crate::kind::Collection;
use lookup::lookup_v2::OwnedSegment;
use once_cell::sync::Lazy;
use regex::Regex;

/// A `field` type that can be used in `Collection<Field>`
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Field(String);

static VALID_FIELD: Lazy<Regex> =
    Lazy::new(|| Regex::new("^[0-9]*[a-zA-Z_@][0-9a-zA-Z_@]*$").unwrap());

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // This can eventually just parse the field and see if it's valid, but the
        // parser is currently lenient in what it accepts so it doesn't catch all errors that
        // should be quoted
        let needs_quotes = !VALID_FIELD.is_match(&self.0);
        if needs_quotes {
            write!(f, "\"{}\"", self.0)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

impl CollectionKey for Field {
    fn to_segment(&self) -> OwnedSegment {
        OwnedSegment::Field(self.0.clone())
    }
}

impl Field {
    /// Get a `str` representation of the field.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl CollectionRemove for Collection<Field> {
    type Key = Field;

    fn remove_known(&mut self, key: &Field) {
        self.known.remove(key);
    }
}

impl std::ops::Deref for Field {
    type Target = String;

    fn deref(&self) -> &String {
        &self.0
    }
}

impl From<&str> for Field {
    fn from(field: &str) -> Self {
        Self(field.into())
    }
}

impl From<String> for Field {
    fn from(field: String) -> Self {
        Self(field)
    }
}
