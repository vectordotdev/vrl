//! The `ObjectMap` type: storage for [`Value::Object`].
//!
//! Entries are stored inline in a single `Vec`, sorted by key. Typical event
//! objects are small, so this is more compact and cache friendly than a tree,
//! and iteration order stays sorted by key.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use super::{KeyString, Value};

type Entry = (KeyString, Value);

/// Maps at or below this size use a linear scan for lookups, larger maps use
/// binary search.
const LINEAR_SCAN_MAX: usize = 16;

/// Bulk-built and cloned maps of at least this size get spare capacity for a
/// few inserts.
const SLACK_MIN_LEN: usize = 16;

/// Capacity for a bulk-built or cloned map of `len` entries. Exact capacity would
/// make the first added field (e.g. source metadata on a decoded event, or a
/// remap after fan-out) double the allocation and copy every entry.
#[inline]
fn capacity_with_slack(len: usize) -> usize {
    if len >= SLACK_MIN_LEN {
        len + (len / 32).max(4)
    } else {
        len
    }
}

/// The storage mapping for the `Object` variant.
///
/// Entries are kept sorted by key, so iteration order is sorted.
#[derive(Default)]
pub struct ObjectMap {
    entries: Vec<Entry>,
}

impl Clone for ObjectMap {
    fn clone(&self) -> Self {
        let mut entries = Vec::with_capacity(capacity_with_slack(self.entries.len()));
        entries.extend_from_slice(&self.entries);
        Self { entries }
    }
}

impl ObjectMap {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Build from entries in arbitrary order. Later duplicates win, matching
    /// repeated `insert`.
    fn from_unsorted(mut entries: Vec<Entry>) -> Self {
        if !entries.windows(2).all(|w| w[0].0 < w[1].0) {
            // Stable sort keeps duplicates in insertion order, so keep the last one.
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut write = 0;
            for read in 1..entries.len() {
                if entries[read].0 != entries[write].0 {
                    write += 1;
                }
                entries.swap(read, write);
            }
            entries.truncate(write + 1);
        }
        Self { entries }
    }

    /// Binary search for `key` in the sorted entries.
    #[inline]
    fn search(&self, key: &str) -> Result<usize, usize> {
        self.entries.binary_search_by(|(k, _)| k.as_str().cmp(key))
    }

    /// Find the position of `key` for lookups.
    #[inline]
    fn position(&self, key: &str) -> Option<usize> {
        if self.entries.len() <= LINEAR_SCAN_MAX {
            self.entries.iter().position(|(k, _)| k.as_str() == key)
        } else {
            self.search(key).ok()
        }
    }

    #[inline]
    pub fn insert(&mut self, key: KeyString, value: Value) -> Option<Value> {
        match self.search(key.as_str()) {
            Ok(pos) => Some(std::mem::replace(&mut self.entries[pos].1, value)),
            Err(pos) => {
                self.entries.insert(pos, (key, value));
                None
            }
        }
    }

    #[inline]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.position(key).map(|pos| &self.entries[pos].1)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        let pos = self.position(key)?;
        Some(&mut self.entries[pos].1)
    }

    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let pos = self.position(key)?;
        Some(self.entries.remove(pos).1)
    }

    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.position(key).is_some()
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub fn keys(&self) -> ObjectMapKeys<'_> {
        ObjectMapKeys(self.entries.iter())
    }

    #[must_use]
    pub fn values(&self) -> ObjectMapValues<'_> {
        ObjectMapValues(self.entries.iter())
    }

    pub fn values_mut(&mut self) -> ObjectMapValuesMut<'_> {
        ObjectMapValuesMut(self.entries.iter_mut())
    }

    #[must_use]
    pub fn iter(&self) -> ObjectMapIter<'_> {
        ObjectMapIter(self.entries.iter())
    }

    pub fn iter_mut(&mut self) -> ObjectMapIterMut<'_> {
        ObjectMapIterMut(self.entries.iter_mut())
    }

    #[must_use]
    pub fn into_keys(self) -> ObjectMapIntoKeys {
        ObjectMapIntoKeys(self.entries.into_iter())
    }

    #[must_use]
    pub fn into_values(self) -> ObjectMapIntoValues {
        ObjectMapIntoValues(self.entries.into_iter())
    }

    #[inline]
    pub fn entry(&mut self, key: KeyString) -> ObjectMapEntry<'_> {
        match self.search(key.as_str()) {
            Ok(index) => ObjectMapEntry::Occupied(ObjectMapOccupiedEntry {
                entries: &mut self.entries,
                index,
            }),
            Err(index) => ObjectMapEntry::Vacant(ObjectMapVacantEntry {
                entries: &mut self.entries,
                index,
                key,
            }),
        }
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&KeyString, &mut Value) -> bool,
    {
        self.entries.retain_mut(|(k, v)| f(k, v));
    }

    /// Insert a new empty child `ObjectMap` at the given key and return a
    /// mutable reference to it.
    ///
    /// # Panics
    ///
    /// Panics if the inserted child value is unexpectedly not an object. This
    /// should be unreachable because the value is constructed immediately before
    /// insertion.
    pub fn insert_child(&mut self, key: KeyString) -> &mut ObjectMap {
        let child = Value::Object(ObjectMap::new());
        let index = match self.search(key.as_str()) {
            Ok(index) => {
                self.entries[index].1 = child;
                index
            }
            Err(index) => {
                self.entries.insert(index, (key, child));
                index
            }
        };
        match &mut self.entries[index].1 {
            Value::Object(map) => map,
            _ => unreachable!("inserted child value must be an object"),
        }
    }
}

/// Upper bound on the initial allocation of an `UnsortedBuilder`.
const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

/// Collects entries in arbitrary order for bulk construction.
pub(crate) struct UnsortedBuilder(Vec<Entry>);

impl UnsortedBuilder {
    /// `capacity` may come from untrusted input (e.g. a deserializer's size hint
    /// read from a CBOR map header), so the initial allocation is capped and
    /// larger maps grow as entries are pushed.
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.min(MAX_PREALLOC_BYTES / size_of::<Entry>());
        Self(Vec::with_capacity(capacity_with_slack(capacity)))
    }

    #[inline]
    pub(crate) fn push(&mut self, key: KeyString, value: Value) {
        self.0.push((key, value));
    }

    #[inline]
    pub(crate) fn finish(self) -> ObjectMap {
        ObjectMap::from_unsorted(self.0)
    }
}

// --- Iterators ---

pub struct ObjectMapKeys<'a>(std::slice::Iter<'a, Entry>);

impl<'a> Iterator for ObjectMapKeys<'a> {
    type Item = &'a KeyString;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapValues<'a>(std::slice::Iter<'a, Entry>);

impl<'a> Iterator for ObjectMapValues<'a> {
    type Item = &'a Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapValuesMut<'a>(std::slice::IterMut<'a, Entry>);

impl<'a> Iterator for ObjectMapValuesMut<'a> {
    type Item = &'a mut Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

#[derive(Clone, Debug)]
pub struct ObjectMapIter<'a>(std::slice::Iter<'a, Entry>);

impl<'a> Iterator for ObjectMapIter<'a> {
    type Item = (&'a KeyString, &'a Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for ObjectMapIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, v)| (k, v))
    }
}

impl ExactSizeIterator for ObjectMapIter<'_> {}

pub struct ObjectMapIterMut<'a>(std::slice::IterMut<'a, Entry>);

impl<'a> Iterator for ObjectMapIterMut<'a> {
    type Item = (&'a KeyString, &'a mut Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (&*k, v))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoKeys(std::vec::IntoIter<Entry>);

impl Iterator for ObjectMapIntoKeys {
    type Item = KeyString;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoValues(std::vec::IntoIter<Entry>);

impl Iterator for ObjectMapIntoValues {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoIter(std::vec::IntoIter<Entry>);

impl Iterator for ObjectMapIntoIter {
    type Item = (KeyString, Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

// --- Entry API ---

pub enum ObjectMapEntry<'a> {
    Occupied(ObjectMapOccupiedEntry<'a>),
    Vacant(ObjectMapVacantEntry<'a>),
}

pub struct ObjectMapOccupiedEntry<'a> {
    entries: &'a mut Vec<Entry>,
    index: usize,
}

impl<'a> ObjectMapOccupiedEntry<'a> {
    #[inline]
    #[must_use]
    pub fn get(&self) -> &Value {
        &self.entries[self.index].1
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.entries[self.index].1
    }

    #[inline]
    pub fn insert(&mut self, value: Value) -> Value {
        std::mem::replace(self.get_mut(), value)
    }

    #[inline]
    #[must_use]
    pub fn into_mut(self) -> &'a mut Value {
        &mut self.entries[self.index].1
    }

    #[inline]
    #[must_use]
    pub fn remove_entry(self) -> (KeyString, Value) {
        self.entries.remove(self.index)
    }
}

pub struct ObjectMapVacantEntry<'a> {
    entries: &'a mut Vec<Entry>,
    index: usize,
    key: KeyString,
}

impl<'a> ObjectMapVacantEntry<'a> {
    #[inline]
    pub fn insert(self, value: Value) -> &'a mut Value {
        self.entries.insert(self.index, (self.key, value));
        &mut self.entries[self.index].1
    }
}

impl<'a> ObjectMapEntry<'a> {
    #[inline]
    #[must_use]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut Value),
    {
        match self {
            Self::Occupied(mut entry) => {
                f(entry.get_mut());
                Self::Occupied(entry)
            }
            Self::Vacant(entry) => Self::Vacant(entry),
        }
    }

    #[inline]
    pub fn or_insert(self, default: Value) -> &'a mut Value {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    #[inline]
    pub fn or_insert_with<F>(self, default: F) -> &'a mut Value
    where
        F: FnOnce() -> Value,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(entry) => entry.insert(default()),
        }
    }
}

// --- Trait implementations ---

impl fmt::Debug for ObjectMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

// Entries are sorted by key, so comparing, hashing, and ordering entry-wise
// matches the semantics of a sorted map.

impl PartialEq for ObjectMap {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for ObjectMap {}

impl Hash for ObjectMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Length prefix, like `BTreeMap`, so nested collections hash prefix-free.
        state.write_usize(self.entries.len());
        for (key, value) in &self.entries {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl PartialOrd for ObjectMap {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.iter().partial_cmp(other.iter())
    }
}

impl IntoIterator for ObjectMap {
    type Item = (KeyString, Value);
    type IntoIter = ObjectMapIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        ObjectMapIntoIter(self.entries.into_iter())
    }
}

impl<'a> IntoIterator for &'a ObjectMap {
    type Item = (&'a KeyString, &'a Value);
    type IntoIter = ObjectMapIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut ObjectMap {
    type Item = (&'a KeyString, &'a mut Value);
    type IntoIter = ObjectMapIterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl FromIterator<(KeyString, Value)> for ObjectMap {
    fn from_iter<I: IntoIterator<Item = (KeyString, Value)>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut entries = Vec::with_capacity(capacity_with_slack(iter.size_hint().0));
        entries.extend(iter);
        Self::from_unsorted(entries)
    }
}

impl Extend<(KeyString, Value)> for ObjectMap {
    fn extend<I: IntoIterator<Item = (KeyString, Value)>>(&mut self, iter: I) {
        if self.entries.is_empty() {
            *self = iter.into_iter().collect();
        } else {
            for (k, v) in iter {
                self.insert(k, v);
            }
        }
    }
}

impl<const N: usize> From<[(KeyString, Value); N]> for ObjectMap {
    #[inline]
    fn from(arr: [(KeyString, Value); N]) -> Self {
        arr.into_iter().collect()
    }
}

impl From<BTreeMap<KeyString, Value>> for ObjectMap {
    #[inline]
    fn from(map: BTreeMap<KeyString, Value>) -> Self {
        // BTreeMap iterates in sorted order without duplicates.
        Self {
            entries: map.into_iter().collect(),
        }
    }
}

impl std::ops::Index<&str> for ObjectMap {
    type Output = Value;

    #[inline]
    fn index(&self, key: &str) -> &Value {
        self.get(key).expect("key not found")
    }
}

impl std::ops::Index<&KeyString> for ObjectMap {
    type Output = Value;

    #[inline]
    fn index(&self, key: &KeyString) -> &Value {
        self.get(key.as_ref()).expect("key not found")
    }
}

#[cfg(any(test, feature = "test"))]
impl PartialEq<BTreeMap<KeyString, Value>> for ObjectMap {
    #[inline]
    fn eq(&self, other: &BTreeMap<KeyString, Value>) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

#[cfg(any(test, feature = "test"))]
impl PartialEq<ObjectMap> for BTreeMap<KeyString, Value> {
    #[inline]
    fn eq(&self, other: &ObjectMap) -> bool {
        other == self
    }
}

impl ::serde::Serialize for ObjectMap {
    fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.iter())
    }
}

impl<'de> ::serde::Deserialize<'de> for ObjectMap {
    fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor;

        impl<'de> ::serde::de::Visitor<'de> for MapVisitor {
            type Value = ObjectMap;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A: ::serde::de::MapAccess<'de>>(
                self,
                mut access: A,
            ) -> Result<ObjectMap, A::Error> {
                let mut builder = UnsortedBuilder::with_capacity(access.size_hint().unwrap_or(0));
                while let Some((key, value)) = access.next_entry()? {
                    builder.push(key, value);
                }
                Ok(builder.finish())
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(map: &ObjectMap) -> Vec<&str> {
        map.keys().map(KeyString::as_str).collect()
    }

    #[test]
    fn insert_keeps_keys_sorted() {
        let mut map = ObjectMap::new();
        for key in ["delta", "alpha", "charlie", "bravo"] {
            assert!(map.insert(key.into(), Value::from(key)).is_none());
        }
        assert_eq!(keys(&map), ["alpha", "bravo", "charlie", "delta"]);
        assert_eq!(
            map.insert("bravo".into(), Value::from(2)),
            Some(Value::from("bravo"))
        );
        assert_eq!(map.get("bravo"), Some(&Value::from(2)));
    }

    #[test]
    fn from_iter_sorts_and_keeps_last_duplicate() {
        let map: ObjectMap = [
            ("b".into(), Value::from(1)),
            ("a".into(), Value::from(2)),
            ("b".into(), Value::from(3)),
            ("c".into(), Value::from(4)),
            ("a".into(), Value::from(5)),
        ]
        .into_iter()
        .collect();
        assert_eq!(keys(&map), ["a", "b", "c"]);
        assert_eq!(map["a"], Value::from(5));
        assert_eq!(map["b"], Value::from(3));
        assert_eq!(map["c"], Value::from(4));
    }

    #[test]
    fn lookups_use_binary_search_for_large_maps() {
        let n = LINEAR_SCAN_MAX * 4;
        let map: ObjectMap = (0..n)
            .rev()
            .map(|i| (KeyString::from(format!("key{i:03}")), Value::from(i as i64)))
            .collect();
        assert_eq!(map.len(), n);
        for i in 0..n {
            assert_eq!(map.get(&format!("key{i:03}")), Some(&Value::from(i as i64)));
        }
        assert_eq!(map.get("missing"), None);
    }

    #[test]
    fn deserialize_matches_insert_semantics() {
        let map: ObjectMap = serde_json::from_str(r#"{"b": 1, "a": 2, "b": 3}"#).unwrap();
        assert_eq!(keys(&map), ["a", "b"]);
        assert_eq!(map["b"], Value::from(3));
    }
}
