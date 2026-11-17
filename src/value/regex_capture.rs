//! Engine-independent capture views and materialization into ordinary VRL values.
use bytes::Bytes;

use super::{KeyString, ObjectMap, Value};

/// Allocation policy for a regex capture object. Value semantics are identical.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RegexCaptureMapMode {
    /// Copy each captured string independently, preserving existing retention.
    #[default]
    Owned,
    /// Copy the full match once and share its backing bytes between captures.
    Shared,
}

/// A borrowed match; offsets are relative to the complete searched subject.
#[derive(Clone, Copy, Debug)]
pub struct CaptureView<'a> {
    pub start: usize,
    pub end: usize,
    pub text: &'a str,
}

/// Minimal capture interface required by VRL's map conversion.
pub trait CaptureGroups {
    fn group_len(&self) -> usize;
    fn group(&self, index: usize) -> Option<CaptureView<'_>>;
}

/// Borrowed capture offsets from a reusable matching scratch buffer.
pub struct LocatedCaptures<'a> {
    subject: &'a str,
    locations: &'a regex::CaptureLocations,
}

impl<'a> LocatedCaptures<'a> {
    #[must_use]
    pub fn new(subject: &'a str, locations: &'a regex::CaptureLocations) -> Self {
        Self { subject, locations }
    }
}

impl CaptureGroups for LocatedCaptures<'_> {
    fn group_len(&self) -> usize {
        self.locations.len()
    }

    fn group(&self, index: usize) -> Option<CaptureView<'_>> {
        self.locations.get(index).map(|(start, end)| CaptureView {
            start,
            end,
            text: &self.subject[start..end],
        })
    }
}

impl CaptureGroups for regex::Captures<'_> {
    fn group_len(&self) -> usize {
        self.len()
    }

    fn group(&self, index: usize) -> Option<CaptureView<'_>> {
        self.get(index).map(|m| CaptureView {
            start: m.start(),
            end: m.end(),
            text: m.as_str(),
        })
    }
}

/// A short-lived map proxy. It borrows capture metadata only while constructing
/// the normal owned `ObjectMap`; no borrowed data escapes in a VRL `Value`.
pub struct RegexCaptureMap<'a, C: CaptureGroups + ?Sized> {
    captures: &'a C,
    backing: Option<(usize, Bytes)>,
}

impl<'a, C: CaptureGroups + ?Sized> RegexCaptureMap<'a, C> {
    pub fn new(captures: &'a C, mode: RegexCaptureMapMode, has_fields: bool) -> Self {
        let backing = (mode == RegexCaptureMapMode::Shared && has_fields)
            .then(|| captures.group(0))
            .flatten()
            .map(|full| (full.start, Bytes::copy_from_slice(full.text.as_bytes())));
        Self { captures, backing }
    }

    fn value(&self, index: usize) -> Value {
        let Some(group) = self.captures.group(index) else {
            return Value::Null;
        };
        match &self.backing {
            Some((base, bytes)) if group.start >= *base && group.end - base <= bytes.len() => {
                Value::Bytes(bytes.slice(group.start - base..group.end - base))
            }
            // Independent copying also handles captures outside the full match
            // if a future backend permits them (for example in lookarounds).
            _ => Value::Bytes(Bytes::copy_from_slice(group.text.as_bytes())),
        }
    }

    pub fn materialize(self, names: &[(KeyString, usize)], numeric_groups: bool) -> ObjectMap {
        let named_fields = names
            .iter()
            .map(|(name, index)| (name.clone(), self.value(*index)));
        if numeric_groups {
            // Preserve the current VRL numbering behavior, including the compact
            // numbering of participating numeric groups after an optional miss.
            let numeric = (0..self.captures.group_len())
                .filter(|&index| self.captures.group(index).is_some())
                .enumerate()
                .map(|(name, index)| (KeyString::from(name.to_string()), self.value(index)));
            numeric.chain(named_fields).collect()
        } else {
            named_fields.collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_and_owned_preserve_unicode_empty_optional_and_numeric_values() {
        for pattern in [
            r"(?P<a>é)(?P<b>x)?(?P<c>)",
            r"(?P<one>one) (?P<two>two)",
            r".*",
        ] {
            let regex = regex::Regex::new(pattern).unwrap();
            let names: Vec<_> = regex
                .capture_names()
                .enumerate()
                .filter_map(|(i, name)| name.map(|n| (KeyString::from(n), i)))
                .collect();
            for input in ["before é after", "one two", "", "a\0b"] {
                if let Some(captures) = regex.captures(input) {
                    for numeric in [false, true] {
                        let owned = RegexCaptureMap::new(
                            &captures,
                            RegexCaptureMapMode::Owned,
                            numeric || !names.is_empty(),
                        )
                        .materialize(&names, numeric);
                        let shared = RegexCaptureMap::new(
                            &captures,
                            RegexCaptureMapMode::Shared,
                            numeric || !names.is_empty(),
                        )
                        .materialize(&names, numeric);
                        assert_eq!(shared, owned);
                    }
                }
            }
        }
    }

    #[test]
    fn named_capture_slices_share_one_owned_match_and_outlive_input() {
        let map = {
            let input = String::from("prefix one two suffix");
            let regex = regex::Regex::new(r"(?P<a>one) (?P<b>two)").unwrap();
            let captures = regex.captures(&input).unwrap();
            RegexCaptureMap::new(&captures, RegexCaptureMapMode::Shared, true)
                .materialize(&[("a".into(), 1), ("b".into(), 2)], false)
        };
        let a = map["a"].as_bytes().unwrap();
        let b = map["b"].as_bytes().unwrap();
        assert_eq!(a.as_ref(), b"one");
        assert_eq!(b.as_ref(), b"two");
        assert_eq!(b.as_ptr() as usize - a.as_ptr() as usize, 4);
    }
}
