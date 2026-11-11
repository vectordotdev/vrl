//! The `ObjectMap` type: storage for [`Value::Object`].
//!
//! `ObjectMap` is an opaque map from [`KeyString`] to [`Value`] whose iteration
//! order is sorted by key. Its internal representation is private so that it can
//! change for efficiency without breaking callers.

use std::cmp::Ordering;
use std::collections::{BTreeMap, btree_map};
use std::fmt;
use std::hash::{Hash, Hasher};

use super::{KeyString, Value};

/// The storage mapping for the `Object` variant.
///
/// Iteration order is sorted by key.
#[derive(Clone, Default)]
pub struct ObjectMap {
    map: BTreeMap<KeyString, Value>,
}

impl ObjectMap {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn insert(&mut self, key: KeyString, value: Value) -> Option<Value> {
        self.map.insert(key, value)
    }

    #[inline]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.map.get_mut(key)
    }

    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.map.remove(key)
    }

    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    #[must_use]
    pub fn keys(&self) -> ObjectMapKeys<'_> {
        ObjectMapKeys(self.map.keys())
    }

    #[must_use]
    pub fn values(&self) -> ObjectMapValues<'_> {
        ObjectMapValues(self.map.values())
    }

    pub fn values_mut(&mut self) -> ObjectMapValuesMut<'_> {
        ObjectMapValuesMut(self.map.values_mut())
    }

    #[must_use]
    pub fn iter(&self) -> ObjectMapIter<'_> {
        ObjectMapIter(self.map.iter())
    }

    pub fn iter_mut(&mut self) -> ObjectMapIterMut<'_> {
        ObjectMapIterMut(self.map.iter_mut())
    }

    #[must_use]
    pub fn into_keys(self) -> ObjectMapIntoKeys {
        ObjectMapIntoKeys(self.map.into_keys())
    }

    #[must_use]
    pub fn into_values(self) -> ObjectMapIntoValues {
        ObjectMapIntoValues(self.map.into_values())
    }

    #[inline]
    pub fn entry(&mut self, key: KeyString) -> ObjectMapEntry<'_> {
        match self.map.entry(key) {
            btree_map::Entry::Occupied(entry) => {
                ObjectMapEntry::Occupied(ObjectMapOccupiedEntry(entry))
            }
            btree_map::Entry::Vacant(entry) => ObjectMapEntry::Vacant(ObjectMapVacantEntry(entry)),
        }
    }

    #[inline]
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&KeyString, &mut Value) -> bool,
    {
        self.map.retain(|k, v| f(k, v));
    }

    /// Insert a new empty child `ObjectMap` at the given key and return a
    /// mutable reference to it.
    ///
    /// # Panics
    ///
    /// Panics if the inserted child value is unexpectedly not an object. This
    /// should be unreachable because the value is constructed immediately before
    /// insertion.
    #[inline]
    pub fn insert_child(&mut self, key: KeyString) -> &mut ObjectMap {
        let child = Value::Object(ObjectMap::new());
        let slot = match self.map.entry(key) {
            btree_map::Entry::Occupied(mut entry) => {
                entry.insert(child);
                entry.into_mut()
            }
            btree_map::Entry::Vacant(entry) => entry.insert(child),
        };
        match slot {
            Value::Object(map) => map,
            _ => unreachable!("inserted child value must be an object"),
        }
    }
}

// --- Iterators ---

pub struct ObjectMapKeys<'a>(btree_map::Keys<'a, KeyString, Value>);

impl<'a> Iterator for ObjectMapKeys<'a> {
    type Item = &'a KeyString;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapValues<'a>(btree_map::Values<'a, KeyString, Value>);

impl<'a> Iterator for ObjectMapValues<'a> {
    type Item = &'a Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapValuesMut<'a>(btree_map::ValuesMut<'a, KeyString, Value>);

impl<'a> Iterator for ObjectMapValuesMut<'a> {
    type Item = &'a mut Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

#[derive(Clone, Debug)]
pub struct ObjectMapIter<'a>(btree_map::Iter<'a, KeyString, Value>);

impl<'a> Iterator for ObjectMapIter<'a> {
    type Item = (&'a KeyString, &'a Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for ObjectMapIter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl ExactSizeIterator for ObjectMapIter<'_> {}

pub struct ObjectMapIterMut<'a>(btree_map::IterMut<'a, KeyString, Value>);

impl<'a> Iterator for ObjectMapIterMut<'a> {
    type Item = (&'a KeyString, &'a mut Value);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoKeys(btree_map::IntoKeys<KeyString, Value>);

impl Iterator for ObjectMapIntoKeys {
    type Item = KeyString;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoValues(btree_map::IntoValues<KeyString, Value>);

impl Iterator for ObjectMapIntoValues {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ObjectMapIntoIter(btree_map::IntoIter<KeyString, Value>);

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

pub struct ObjectMapOccupiedEntry<'a>(btree_map::OccupiedEntry<'a, KeyString, Value>);

impl<'a> ObjectMapOccupiedEntry<'a> {
    #[inline]
    #[must_use]
    pub fn get(&self) -> &Value {
        self.0.get()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut Value {
        self.0.get_mut()
    }

    #[inline]
    pub fn insert(&mut self, value: Value) -> Value {
        self.0.insert(value)
    }

    #[inline]
    #[must_use]
    pub fn into_mut(self) -> &'a mut Value {
        self.0.into_mut()
    }

    #[inline]
    #[must_use]
    pub fn remove_entry(self) -> (KeyString, Value) {
        self.0.remove_entry()
    }
}

pub struct ObjectMapVacantEntry<'a>(btree_map::VacantEntry<'a, KeyString, Value>);

impl<'a> ObjectMapVacantEntry<'a> {
    #[inline]
    pub fn insert(self, value: Value) -> &'a mut Value {
        self.0.insert(value)
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
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.map.fmt(f)
    }
}

impl PartialEq for ObjectMap {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.map == other.map
    }
}

impl Eq for ObjectMap {}

impl Hash for ObjectMap {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.map.hash(state);
    }
}

impl PartialOrd for ObjectMap {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.map.partial_cmp(&other.map)
    }
}

impl IntoIterator for ObjectMap {
    type Item = (KeyString, Value);
    type IntoIter = ObjectMapIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        ObjectMapIntoIter(self.map.into_iter())
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
    #[inline]
    fn from_iter<I: IntoIterator<Item = (KeyString, Value)>>(iter: I) -> Self {
        Self {
            map: BTreeMap::from_iter(iter),
        }
    }
}

impl Extend<(KeyString, Value)> for ObjectMap {
    #[inline]
    fn extend<I: IntoIterator<Item = (KeyString, Value)>>(&mut self, iter: I) {
        self.map.extend(iter);
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
        Self { map }
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
        self.map == *other
    }
}

#[cfg(any(test, feature = "test"))]
impl PartialEq<ObjectMap> for BTreeMap<KeyString, Value> {
    #[inline]
    fn eq(&self, other: &ObjectMap) -> bool {
        *self == other.map
    }
}

impl ::serde::Serialize for ObjectMap {
    #[inline]
    fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.map.serialize(serializer)
    }
}

impl<'de> ::serde::Deserialize<'de> for ObjectMap {
    #[inline]
    fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::deserialize(deserializer).map(|map| Self { map })
    }
}
