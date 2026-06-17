//! Contains the main "Value" type for Vector and VRL, as well as helper methods.

#[allow(clippy::module_name_repetitions)]
pub use super::value::regex::ValueRegex;
#[allow(clippy::module_name_repetitions)]
pub use iter::{IterItem, ValueIter};

use bytes::{Bytes, BytesMut};
use chrono::{DateTime, SecondsFormat, Utc};
use ecow::EcoVec;
use ordered_float::NotNan;
use std::borrow::Cow;
use std::fmt;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, btree_map},
    hash::{Hash, Hasher},
};

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
///
/// This is a newtype wrapper around `BTreeMap<KeyString, Value>` that preserves
/// sorted iteration order. The internal representation is opaque and may change
/// for efficiency in the future.
pub enum ObjectMap {
    BTree(BTreeMap<KeyString, Value>),
    Flat(EcoVec<(KeyString, Value)>),
    VecFlat(Vec<(KeyString, Value)>),
}

impl Clone for ObjectMap {
    fn clone(&self) -> Self {
        match self {
            Self::BTree(map) => Self::BTree(map.clone()),
            // O(1): EcoVec clone just increments a refcount.
            Self::Flat(vec) => Self::Flat(vec.clone()),
            Self::VecFlat(vec) => Self::VecFlat(vec.clone()),
        }
    }
}

impl fmt::Debug for ObjectMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BTree(map) => f.debug_map().entries(map.iter()).finish(),
            _ => {
                let s = self.as_flat_slice().unwrap();
                f.debug_map()
                    .entries(s.iter().map(|(k, v)| (k, v)))
                    .finish()
            }
        }
    }
}

impl Default for ObjectMap {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ObjectMapKeys<'a> {
    BTree(btree_map::Keys<'a, KeyString, Value>),
    Flat(std::slice::Iter<'a, (KeyString, Value)>),
}

impl<'a> Iterator for ObjectMapKeys<'a> {
    type Item = &'a KeyString;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(k, _)| k),
        }
    }
}

pub enum ObjectMapValues<'a> {
    BTree(btree_map::Values<'a, KeyString, Value>),
    Flat(std::slice::Iter<'a, (KeyString, Value)>),
}

impl<'a> Iterator for ObjectMapValues<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(_, v)| v),
        }
    }
}

pub enum ObjectMapValuesMut<'a> {
    BTree(btree_map::ValuesMut<'a, KeyString, Value>),
    Flat(std::slice::IterMut<'a, (KeyString, Value)>),
}

impl<'a> Iterator for ObjectMapValuesMut<'a> {
    type Item = &'a mut Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(_, v)| v),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ObjectMapIter<'a> {
    BTree(btree_map::Iter<'a, KeyString, Value>),
    Flat(std::slice::Iter<'a, (KeyString, Value)>),
}

impl<'a> Iterator for ObjectMapIter<'a> {
    type Item = (&'a KeyString, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(k, v)| (k, v)),
        }
    }
}

pub enum ObjectMapIterMut<'a> {
    BTree(btree_map::IterMut<'a, KeyString, Value>),
    Flat(std::slice::IterMut<'a, (KeyString, Value)>),
}

impl<'a> Iterator for ObjectMapIterMut<'a> {
    type Item = (&'a KeyString, &'a mut Value);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(k, v)| (k as &KeyString, v)),
        }
    }
}

pub enum ObjectMapIntoKeys {
    BTree(btree_map::IntoKeys<KeyString, Value>),
    Flat(std::vec::IntoIter<(KeyString, Value)>),
}

impl Iterator for ObjectMapIntoKeys {
    type Item = KeyString;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(k, _)| k),
        }
    }
}

pub enum ObjectMapIntoValues {
    BTree(btree_map::IntoValues<KeyString, Value>),
    Flat(std::vec::IntoIter<(KeyString, Value)>),
}

impl Iterator for ObjectMapIntoValues {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next().map(|(_, v)| v),
        }
    }
}

pub enum ObjectMapIntoIter {
    BTree(btree_map::IntoIter<KeyString, Value>),
    Flat(std::vec::IntoIter<(KeyString, Value)>),
}

impl Iterator for ObjectMapIntoIter {
    type Item = (KeyString, Value);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BTree(iter) => iter.next(),
            Self::Flat(iter) => iter.next(),
        }
    }
}

pub enum ObjectMapEntry<'a> {
    Occupied(ObjectMapOccupiedEntry<'a>),
    Vacant(ObjectMapVacantEntry<'a>),
}

pub struct FlatOccupiedEntry<'a> {
    vec: &'a mut EcoVec<(KeyString, Value)>,
    index: usize,
}

impl<'a> FlatOccupiedEntry<'a> {
    pub fn get(&self) -> &Value {
        &self.vec[self.index].1
    }

    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.vec.make_mut()[self.index].1
    }

    pub fn insert(&mut self, value: Value) -> Value {
        std::mem::replace(&mut self.vec.make_mut()[self.index].1, value)
    }

    pub fn into_mut(self) -> &'a mut Value {
        &mut self.vec.make_mut()[self.index].1
    }
}

pub struct FlatVacantEntry<'a> {
    vec: &'a mut EcoVec<(KeyString, Value)>,
    key: KeyString,
}

impl<'a> FlatVacantEntry<'a> {
    pub fn insert(self, value: Value) -> &'a mut Value {
        let idx = self
            .vec
            .binary_search_by(|(k, _)| k.cmp(&self.key))
            .unwrap_or_else(|idx| idx);
        self.vec.insert(idx, (self.key, value));
        &mut self.vec.make_mut()[idx].1
    }
}

pub struct VecFlatOccupiedEntry<'a> {
    vec: &'a mut Vec<(KeyString, Value)>,
    index: usize,
}

impl<'a> VecFlatOccupiedEntry<'a> {
    pub fn get(&self) -> &Value {
        &self.vec[self.index].1
    }
    pub fn get_mut(&mut self) -> &mut Value {
        &mut self.vec[self.index].1
    }
    pub fn insert(&mut self, value: Value) -> Value {
        std::mem::replace(&mut self.vec[self.index].1, value)
    }
    pub fn into_mut(self) -> &'a mut Value {
        &mut self.vec[self.index].1
    }
}

pub struct VecFlatVacantEntry<'a> {
    vec: &'a mut Vec<(KeyString, Value)>,
    key: KeyString,
}

impl<'a> VecFlatVacantEntry<'a> {
    pub fn insert(self, value: Value) -> &'a mut Value {
        let idx = self
            .vec
            .binary_search_by(|(k, _)| k.cmp(&self.key))
            .unwrap_or_else(|idx| idx);
        self.vec.insert(idx, (self.key, value));
        &mut self.vec[idx].1
    }
}

pub enum ObjectMapOccupiedEntry<'a> {
    BTree(btree_map::OccupiedEntry<'a, KeyString, Value>),
    Flat(FlatOccupiedEntry<'a>),
    VecFlat(VecFlatOccupiedEntry<'a>),
}

impl ObjectMapOccupiedEntry<'_> {
    pub fn get(&self) -> &Value {
        match self {
            Self::BTree(entry) => entry.get(),
            Self::Flat(entry) => entry.get(),
            Self::VecFlat(entry) => entry.get(),
        }
    }

    pub fn get_mut(&mut self) -> &mut Value {
        match self {
            Self::BTree(entry) => entry.get_mut(),
            Self::Flat(entry) => entry.get_mut(),
            Self::VecFlat(entry) => entry.get_mut(),
        }
    }

    pub fn insert(&mut self, value: Value) -> Value {
        match self {
            Self::BTree(entry) => entry.insert(value),
            Self::Flat(entry) => entry.insert(value),
            Self::VecFlat(entry) => entry.insert(value),
        }
    }
}

pub enum ObjectMapVacantEntry<'a> {
    BTree(btree_map::VacantEntry<'a, KeyString, Value>),
    Flat(FlatVacantEntry<'a>),
    VecFlat(VecFlatVacantEntry<'a>),
}

impl<'a> ObjectMapVacantEntry<'a> {
    pub fn insert(self, value: Value) -> &'a mut Value {
        match self {
            Self::BTree(entry) => entry.insert(value),
            Self::Flat(entry) => entry.insert(value),
            Self::VecFlat(entry) => entry.insert(value),
        }
    }
}

impl<'a> ObjectMapEntry<'a> {
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

    pub fn or_insert(self, default: Value) -> &'a mut Value {
        match self {
            Self::Occupied(entry) => match entry {
                ObjectMapOccupiedEntry::BTree(entry) => entry.into_mut(),
                ObjectMapOccupiedEntry::Flat(entry) => entry.into_mut(),
                ObjectMapOccupiedEntry::VecFlat(entry) => entry.into_mut(),
            },
            Self::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with<F>(self, default: F) -> &'a mut Value
    where
        F: FnOnce() -> Value,
    {
        match self {
            Self::Occupied(entry) => match entry {
                ObjectMapOccupiedEntry::BTree(entry) => entry.into_mut(),
                ObjectMapOccupiedEntry::Flat(entry) => entry.into_mut(),
                ObjectMapOccupiedEntry::VecFlat(entry) => entry.into_mut(),
            },
            Self::Vacant(entry) => entry.insert(default()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedBackend {
    BTree,
    Flat,
    VecFlat,
}

fn selected_backend() -> SelectedBackend {
    use std::sync::OnceLock;
    static SELECTED: OnceLock<SelectedBackend> = OnceLock::new();
    *SELECTED.get_or_init(|| match std::env::var("VRL_OBJECT_MAP").ok().as_deref() {
        Some(s) if s.eq_ignore_ascii_case("btree") => SelectedBackend::BTree,
        Some(s) if s.eq_ignore_ascii_case("vec") || s.eq_ignore_ascii_case("vecflat") => {
            SelectedBackend::VecFlat
        }
        _ => SelectedBackend::Flat,
    })
}

impl ObjectMap {
    fn as_flat_slice(&self) -> Option<&[(KeyString, Value)]> {
        match self {
            Self::Flat(vec) => Some(vec),
            Self::VecFlat(vec) => Some(vec),
            Self::BTree(_) => None,
        }
    }

    #[must_use]
    pub fn new() -> Self {
        match selected_backend() {
            SelectedBackend::BTree => Self::new_btree(),
            SelectedBackend::Flat => Self::Flat(EcoVec::new()),
            SelectedBackend::VecFlat => Self::VecFlat(Vec::new()),
        }
    }

    #[must_use]
    pub fn new_btree() -> Self {
        Self::BTree(BTreeMap::new())
    }

    pub fn insert(&mut self, key: KeyString, value: Value) -> Option<Value> {
        match self {
            Self::BTree(map) => map.insert(key, value),
            Self::Flat(vec) => match vec.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(pos) => Some(std::mem::replace(&mut vec.make_mut()[pos].1, value)),
                Err(pos) => {
                    vec.insert(pos, (key, value));
                    None
                }
            },
            Self::VecFlat(vec) => match vec.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(pos) => Some(std::mem::replace(&mut vec[pos].1, value)),
                Err(pos) => {
                    vec.insert(pos, (key, value));
                    None
                }
            },
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::BTree(map) => map.get(key),
            _ => self
                .as_flat_slice()
                .unwrap()
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v),
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Self::BTree(map) => map.get_mut(key),
            Self::Flat(vec) => {
                let pos = vec.iter().position(|(k, _)| k.as_str() == key)?;
                Some(&mut vec.make_mut()[pos].1)
            }
            Self::VecFlat(vec) => vec
                .iter_mut()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        match self {
            Self::BTree(map) => map.remove(key),
            Self::Flat(vec) => {
                let pos = vec.iter().position(|(k, _)| k.as_str() == key)?;
                Some(vec.remove(pos).1)
            }
            Self::VecFlat(vec) => {
                let pos = vec.iter().position(|(k, _)| k.as_str() == key)?;
                Some(vec.remove(pos).1)
            }
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        match self {
            Self::BTree(map) => map.contains_key(key),
            _ => self
                .as_flat_slice()
                .unwrap()
                .iter()
                .any(|(k, _)| k.as_str() == key),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::BTree(map) => map.len(),
            _ => self.as_flat_slice().unwrap().len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::BTree(map) => map.is_empty(),
            _ => self.as_flat_slice().unwrap().is_empty(),
        }
    }

    pub fn clear(&mut self) {
        match self {
            Self::BTree(map) => map.clear(),
            Self::Flat(vec) => vec.clear(),
            Self::VecFlat(vec) => vec.clear(),
        }
    }

    pub fn keys(&self) -> ObjectMapKeys<'_> {
        match self {
            Self::BTree(map) => ObjectMapKeys::BTree(map.keys()),
            _ => ObjectMapKeys::Flat(self.as_flat_slice().unwrap().iter()),
        }
    }

    pub fn values(&self) -> ObjectMapValues<'_> {
        match self {
            Self::BTree(map) => ObjectMapValues::BTree(map.values()),
            _ => ObjectMapValues::Flat(self.as_flat_slice().unwrap().iter()),
        }
    }

    pub fn values_mut(&mut self) -> ObjectMapValuesMut<'_> {
        match self {
            Self::BTree(map) => ObjectMapValuesMut::BTree(map.values_mut()),
            Self::Flat(vec) => ObjectMapValuesMut::Flat(vec.make_mut().iter_mut()),
            Self::VecFlat(vec) => ObjectMapValuesMut::Flat(vec.iter_mut()),
        }
    }

    pub fn iter(&self) -> ObjectMapIter<'_> {
        match self {
            Self::BTree(map) => ObjectMapIter::BTree(map.iter()),
            _ => ObjectMapIter::Flat(self.as_flat_slice().unwrap().iter()),
        }
    }

    pub fn iter_mut(&mut self) -> ObjectMapIterMut<'_> {
        match self {
            Self::BTree(map) => ObjectMapIterMut::BTree(map.iter_mut()),
            Self::Flat(vec) => ObjectMapIterMut::Flat(vec.make_mut().iter_mut()),
            Self::VecFlat(vec) => ObjectMapIterMut::Flat(vec.iter_mut()),
        }
    }

    pub fn into_keys(self) -> ObjectMapIntoKeys {
        match self {
            Self::BTree(map) => ObjectMapIntoKeys::BTree(map.into_keys()),
            Self::Flat(vec) => {
                ObjectMapIntoKeys::Flat(vec.into_iter().collect::<Vec<_>>().into_iter())
            }
            Self::VecFlat(vec) => ObjectMapIntoKeys::Flat(vec.into_iter()),
        }
    }

    pub fn into_values(self) -> ObjectMapIntoValues {
        match self {
            Self::BTree(map) => ObjectMapIntoValues::BTree(map.into_values()),
            Self::Flat(vec) => {
                ObjectMapIntoValues::Flat(vec.into_iter().collect::<Vec<_>>().into_iter())
            }
            Self::VecFlat(vec) => ObjectMapIntoValues::Flat(vec.into_iter()),
        }
    }

    pub fn entry(&mut self, key: KeyString) -> ObjectMapEntry<'_> {
        match self {
            Self::BTree(map) => match map.entry(key) {
                btree_map::Entry::Occupied(entry) => {
                    ObjectMapEntry::Occupied(ObjectMapOccupiedEntry::BTree(entry))
                }
                btree_map::Entry::Vacant(entry) => {
                    ObjectMapEntry::Vacant(ObjectMapVacantEntry::BTree(entry))
                }
            },
            Self::Flat(vec) => match vec.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(index) => {
                    ObjectMapEntry::Occupied(ObjectMapOccupiedEntry::Flat(FlatOccupiedEntry {
                        vec,
                        index,
                    }))
                }
                Err(_) => {
                    ObjectMapEntry::Vacant(ObjectMapVacantEntry::Flat(FlatVacantEntry { vec, key }))
                }
            },
            Self::VecFlat(vec) => match vec.binary_search_by(|(k, _)| k.cmp(&key)) {
                Ok(index) => ObjectMapEntry::Occupied(ObjectMapOccupiedEntry::VecFlat(
                    VecFlatOccupiedEntry { vec, index },
                )),
                Err(_) => {
                    ObjectMapEntry::Vacant(ObjectMapVacantEntry::VecFlat(VecFlatVacantEntry {
                        vec,
                        key,
                    }))
                }
            },
        }
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&KeyString, &mut Value) -> bool,
    {
        match self {
            Self::BTree(map) => map.retain(f),
            Self::Flat(vec) => {
                let mut i = 0;
                while i < vec.len() {
                    let (ref k, ref mut v) = vec.make_mut()[i];
                    if f(k, v) {
                        i += 1;
                    } else {
                        vec.remove(i);
                    }
                }
            }
            Self::VecFlat(vec) => vec.retain_mut(|(k, v)| f(k, v)),
        }
    }

    /// Insert a new empty child `ObjectMap` at the given key and return a
    /// mutable reference to it.  The child uses the same variant as the parent.
    pub fn insert_child(&mut self, key: KeyString) -> &mut ObjectMap {
        let child = match self {
            Self::BTree(_) => ObjectMap::new_btree(),
            Self::Flat(_) => ObjectMap::Flat(EcoVec::new()),
            Self::VecFlat(_) => ObjectMap::VecFlat(Vec::new()),
        };
        self.insert(key.clone(), Value::Object(child));
        match self.get_mut(key.as_ref()).unwrap() {
            Value::Object(map) => map,
            _ => unreachable!(),
        }
    }
}

// --- Trait implementations ---

impl PartialEq for ObjectMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key.as_ref()) == Some(value))
    }
}

impl Eq for ObjectMap {}

impl Hash for ObjectMap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut entries = self.iter().collect::<Vec<_>>();
        entries.sort_unstable_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));
        for (key, value) in entries {
            key.hash(state);
            value.hash(state);
        }
    }
}

impl PartialOrd for ObjectMap {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut left = self.iter().collect::<Vec<_>>();
        let mut right = other.iter().collect::<Vec<_>>();
        left.sort_unstable_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));
        right.sort_unstable_by(|(left_key, _), (right_key, _)| left_key.cmp(right_key));

        left.partial_cmp(&right)
    }
}

impl IntoIterator for ObjectMap {
    type Item = (KeyString, Value);
    type IntoIter = ObjectMapIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::BTree(map) => ObjectMapIntoIter::BTree(map.into_iter()),
            Self::Flat(vec) => {
                let v: Vec<_> = vec.into_iter().collect();
                ObjectMapIntoIter::Flat(v.into_iter())
            }
            Self::VecFlat(vec) => ObjectMapIntoIter::Flat(vec.into_iter()),
        }
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
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl Extend<(KeyString, Value)> for ObjectMap {
    fn extend<I: IntoIterator<Item = (KeyString, Value)>>(&mut self, iter: I) {
        match self {
            Self::BTree(map) => map.extend(iter),
            Self::Flat(_) | Self::VecFlat(_) => {
                for (k, v) in iter {
                    self.insert(k, v);
                }
            }
        }
    }
}

impl<const N: usize> From<[(KeyString, Value); N]> for ObjectMap {
    fn from(arr: [(KeyString, Value); N]) -> Self {
        arr.into_iter().collect()
    }
}

impl From<BTreeMap<KeyString, Value>> for ObjectMap {
    fn from(map: BTreeMap<KeyString, Value>) -> Self {
        match selected_backend() {
            SelectedBackend::BTree => Self::BTree(map),
            SelectedBackend::Flat => Self::Flat(EcoVec::from(map.into_iter().collect::<Vec<_>>())),
            SelectedBackend::VecFlat => Self::VecFlat(map.into_iter().collect()),
        }
    }
}

impl std::ops::Index<&str> for ObjectMap {
    type Output = Value;

    fn index(&self, key: &str) -> &Value {
        self.get(key).expect("key not found")
    }
}

impl std::ops::Index<&KeyString> for ObjectMap {
    type Output = Value;

    fn index(&self, key: &KeyString) -> &Value {
        self.get(key.as_ref()).expect("key not found")
    }
}

#[cfg(any(test, feature = "test"))]
impl PartialEq<BTreeMap<KeyString, Value>> for ObjectMap {
    fn eq(&self, other: &BTreeMap<KeyString, Value>) -> bool {
        self.len() == other.len()
            && other
                .iter()
                .all(|(key, value)| self.get(key.as_ref()) == Some(value))
    }
}

#[cfg(any(test, feature = "test"))]
impl PartialEq<ObjectMap> for BTreeMap<KeyString, Value> {
    fn eq(&self, other: &ObjectMap) -> bool {
        other.len() == self.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key.as_ref()) == Some(value))
    }
}

impl ::serde::Serialize for ObjectMap {
    fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use ::serde::ser::SerializeMap;
        match self {
            Self::BTree(map) => map.serialize(serializer),
            _ => {
                let s = self.as_flat_slice().unwrap();
                let mut map = serializer.serialize_map(Some(s.len()))?;
                for (k, v) in s {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> ::serde::Deserialize<'de> for ObjectMap {
    fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        BTreeMap::deserialize(deserializer)
            .map(IntoIterator::into_iter)
            .map(Self::from_iter)
    }
}

/// The main value type used in Vector events, and VRL.
#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub enum Value {
    /// Bytes - usually representing a UTF8 String.
    Bytes(Bytes),

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
    /// Returns a string description of the value type
    pub const fn kind_str(&self) -> &str {
        match self {
            Self::Bytes(_) | Self::Regex(_) => "string",
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
    /// Will concatenate `Bytes` and overwrite the rest value kinds.
    pub fn merge(&mut self, incoming: Self) {
        match (self, incoming) {
            (Self::Bytes(self_bytes), Self::Bytes(ref incoming)) => {
                let mut bytes = BytesMut::with_capacity(self_bytes.len() + incoming.len());
                bytes.extend_from_slice(&self_bytes[..]);
                bytes.extend_from_slice(&incoming[..]);
                *self_bytes = bytes.freeze();
            }
            (current, incoming) => *current = incoming,
        }
    }

    /// Return if the node is empty, that is, it is an array or map with no items.
    ///
    /// ```rust
    /// use vrl::value::{Value, ObjectMap};
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
    /// let mut val = Value::from(ObjectMap::default());
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
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return None;
        }
        match (self, other) {
            (Self::Bytes(a), Self::Bytes(b)) => a.partial_cmp(b),
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
    use serde_json::json;

    use crate::path;
    use crate::path::BorrowedSegment;

    use super::*;

    mod corner_cases {
        use super::*;

        #[test]
        fn remove_prune_map_with_map() {
            let mut value = Value::from(ObjectMap::default());
            let key = "foo.bar";
            let marker = Value::from(true);
            assert_eq!(value.insert(key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(key, true), Some(marker));
            assert!(!value.contains("foo"));
        }

        #[test]
        fn remove_prune_map_with_array() {
            let mut value = Value::from(ObjectMap::default());
            let key = "foo[0]";
            let marker = Value::from(true);
            assert_eq!(value.insert(key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(key, true), Some(marker));
            assert!(!value.contains("foo"));
        }

        #[test]
        fn remove_prune_array_with_map() {
            let mut value = Value::from(Vec::<Value>::default());
            let key = "[0].bar";
            let marker = Value::from(true);
            assert_eq!(value.insert(key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(key, true), Some(marker));
            assert!(!value.contains(path!(0)));
        }

        #[test]
        fn remove_prune_array_with_array() {
            let mut value = Value::from(Vec::<Value>::default());
            let key = "[0][0]";
            let marker = Value::from(true);
            assert_eq!(value.insert(key, marker.clone()), None);
            // Since the `foo` map is now empty, this should get cleaned.
            assert_eq!(value.remove(key, true), Some(marker));
            assert!(!value.contains(path!(0)));
        }
    }

    #[test]
    fn quickcheck_value() {
        fn inner(mut path: Vec<BorrowedSegment<'static>>) -> TestResult {
            let mut value = Value::from(ObjectMap::default());
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

    #[test]
    fn object_map_flat_variant_supports_crud() {
        let mut value = Value::Object(ObjectMap::new());

        assert_eq!(value.insert("foo.bar", 1), None);
        assert_eq!(value.get("foo.bar"), Some(&Value::from(1)));
        assert_eq!(value.remove("foo.bar", true), Some(Value::from(1)));
        assert!(!value.contains("foo"));
    }

    #[test]
    fn object_map_equality_ignores_variant() {
        let flat = ObjectMap::from([
            ("alpha".into(), Value::from(1)),
            ("beta".into(), Value::from(2)),
        ]);

        let btree = ObjectMap::BTree(BTreeMap::from([
            ("beta".into(), Value::from(2)),
            ("alpha".into(), Value::from(1)),
        ]));

        assert_eq!(flat, btree);
    }

    #[test]
    fn object_map_partial_ord_uses_sorted_contents() {
        let left = ObjectMap::from([
            ("beta".into(), Value::from(2)),
            ("alpha".into(), Value::from(1)),
        ]);
        let right = ObjectMap::from([
            ("alpha".into(), Value::from(1)),
            ("beta".into(), Value::from(3)),
        ]);

        assert_eq!(left.partial_cmp(&right), Some(Ordering::Less));
    }

    #[test]
    fn object_map_flat_round_trips_through_serde() {
        let map = ObjectMap::from([
            ("beta".into(), Value::from(2)),
            ("alpha".into(), Value::from(1)),
        ]);

        let serialized = serde_json::to_value(&map).unwrap();
        let deserialized: ObjectMap = serde_json::from_value(json!({
            "alpha": 1,
            "beta": 2,
        }))
        .unwrap();

        assert_eq!(serialized, json!({"beta": 2, "alpha": 1}));
        assert_eq!(map, deserialized);
    }

    #[test]
    fn object_map_new_returns_flat() {
        assert!(matches!(ObjectMap::new(), ObjectMap::Flat(_)));
    }

    #[test]
    fn object_map_new_btree_returns_btree() {
        assert!(matches!(ObjectMap::new_btree(), ObjectMap::BTree(_)));
    }
}
