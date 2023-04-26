use std::collections::BTreeMap;
use std::iter::Peekable;

use lookup::{FieldBuf, LookupBuf, SegmentBuf};

use crate::Value;

impl Value {
    /// Insert a value, given the provided path.
    ///
    /// # Examples
    ///
    /// ## Insert At Field
    ///
    /// ```
    /// # use value::Value;
    /// # use lookup::LookupBuf;
    /// # use std::str::FromStr;
    /// # use std::collections::BTreeMap;
    /// # use std::iter::FromIterator;
    ///
    /// let fields = vec![("foo".to_owned(), Value::from("bar"))];
    /// let map = BTreeMap::from_iter(fields.into_iter());
    ///
    /// let mut value = Value::Object(map);
    /// let path = LookupBuf::from_str(".foo").unwrap();
    ///
    /// value.insert_by_path(&path, true.into());
    ///
    /// assert_eq!(
    ///     value.get_by_path(&path),
    ///     Some(&true.into()),
    /// )
    /// ```
    ///
    /// ## Insert Into Array
    ///
    /// ```
    /// # use value::Value;
    /// # use lookup::LookupBuf;
    /// # use std::str::FromStr;
    /// # use std::collections::BTreeMap;
    /// # use std::iter::FromIterator;
    ///
    /// let mut value = Value::Array(vec![Value::Boolean(false), Value::Boolean(true)]);
    /// let path = LookupBuf::from_str("[1].foo").unwrap();
    ///
    /// value.insert_by_path(&path, "bar".into());
    ///
    /// let expected = Value::Array(vec![Value::Boolean(false), Value::Object([("foo".into(), "bar".into())].into())]);
    /// assert_eq!(
    ///     value.get_by_path(&LookupBuf::root()),
    ///     Some(&expected),
    /// )
    /// ```
    ///
    pub fn insert_by_path(&mut self, path: &LookupBuf, new: Self) {
        self.insert_by_segments(path.as_segments().iter().peekable(), new);
    }

    /// Remove a value, given the provided path.
    ///
    /// This works similar to [`Value::get_by_path`], except that it removes the
    /// value at the provided path, instead of returning it.
    ///
    /// The one difference is if a root path (`.`) is provided. In this case,
    /// the [`Value`] object (i.e. "self") is set to `Value::Null`.
    ///
    /// If the `compact` argument is set to `true`, then any `Array` or `Object`
    /// that had one of its elements removed and is now empty, is removed as
    /// well.
    pub fn remove_by_path(&mut self, path: &LookupBuf, compact: bool) -> Option<Self> {
        self.remove_by_segments(path.as_segments().iter().peekable(), compact)
    }

    fn get_by_segment_mut(&mut self, segment: &SegmentBuf) -> Option<&mut Self> {
        match segment {
            SegmentBuf::Field(FieldBuf { name, .. }) => self
                .as_object_mut()
                .and_then(|map| map.get_mut(name.as_str())),
            SegmentBuf::Coalesce(fields) => self.as_object_mut().and_then(|map| {
                fields
                    .iter()
                    .find(|field| map.contains_key(field.as_str()))
                    .and_then(move |field| map.get_mut(field.as_str()))
            }),
            SegmentBuf::Index(index) => self.as_array_mut().and_then(|array| {
                let len = array.len() as isize;
                if *index >= len || index.abs() > len {
                    return None;
                }

                index
                    .checked_rem_euclid(len)
                    .and_then(move |i| array.get_mut(i as usize))
            }),
        }
    }

    fn remove_by_segments<'a, T>(
        &mut self,
        mut segments: Peekable<T>,
        compact: bool,
    ) -> Option<Self>
    where
        T: Iterator<Item = &'a SegmentBuf> + Clone,
    {
        let Some(segment) = segments.next() else {
            return match self {
                Self::Object(v) => {
                    let v = std::mem::take(v);
                    Some(Self::Object(v))
                }
                Self::Array(v) => {
                    let v = std::mem::take(v);
                    Some(Self::Array(v))
                }
                _ => {
                    let v = std::mem::replace(self, Self::Null);
                    Some(v)
                }
            }
        };

        if segments.peek().is_none() {
            return self.remove_by_segment(segment);
        }

        if let Some(value) = self.get_by_segment_mut(segment) {
            let removed_value = value.remove_by_segments(segments, compact);

            match value {
                Self::Object(v) if compact & v.is_empty() => self.remove_by_segment(segment),
                Self::Array(v) if compact & v.is_empty() => self.remove_by_segment(segment),
                _ => None,
            };

            return removed_value;
        }

        None
    }

    fn remove_by_segment(&mut self, segment: &SegmentBuf) -> Option<Self> {
        match segment {
            SegmentBuf::Field(FieldBuf { name, .. }) => self
                .as_object_mut()
                .and_then(|map| map.remove(name.as_str())),

            SegmentBuf::Coalesce(fields) => fields
                .iter()
                .find(|field| {
                    self.as_object()
                        .map(|map| map.contains_key(field.as_str()))
                        .unwrap_or_default()
                })
                .and_then(|field| {
                    self.as_object_mut()
                        .and_then(|map| map.remove(field.as_str()))
                }),
            SegmentBuf::Index(index) => self.as_array_mut().and_then(|array| {
                let len = array.len() as isize;
                if *index >= len || index.abs() > len {
                    return None;
                }

                index
                    .checked_rem_euclid(len)
                    .map(|i| array.remove(i as usize))
            }),
        }
    }

    fn insert_by_segments<'a, T>(&mut self, mut segments: Peekable<T>, new: Self)
    where
        T: Iterator<Item = &'a SegmentBuf> + Clone,
    {
        let Some(segment) = segments.peek() else {return *self = new};

        // As long as the provided segments match the shape of the value, we'll
        // traverse down the tree. Once we encounter a value kind that does not
        // match the requested segment, we'll update the value to match and
        // continue on, until we're able to assign the final `new` value.
        match self.get_by_segment_mut(segment) {
            Some(value) => {
                // We have already consumed this element via a peek.
                let _ = segments.next();
                value.insert_by_segments(segments, new);
            }
            None => self.update_by_segments(segments, new),
        };
    }

    fn update_by_segments<'a, T>(&mut self, mut segments: Peekable<T>, new: Self)
    where
        T: Iterator<Item = &'a SegmentBuf> + Clone,
    {
        let Some(segment) = segments.next() else {return};

        let mut handle_field = |field: &str, new, mut segments: Peekable<T>| {
            let key = field.to_owned();

            // `handle_field` is used to update map values, if the current value
            // isn't a map, we need to make it one.
            if !matches!(self, Self::Object(_)) {
                *self = BTreeMap::default().into();
            }

            let Self::Object(map) = self else {unreachable!("see invariant above")};

            match segments.peek() {
                // If there are no other segments to traverse, we'll add the new
                // value to the current map.
                None => {
                    map.insert(key, new);
                    return;
                }
                // If there are more segments to traverse, insert an empty map
                // or array depending on what the next segment is, and continue
                // to add the next segment.
                Some(next) => match next {
                    SegmentBuf::Index(_) => map.insert(key, Self::Array(vec![])),
                    _ => map.insert(key, BTreeMap::default().into()),
                },
            };

            map.get_mut(field)
                .unwrap()
                .insert_by_segments(segments, new);
        };

        match segment {
            SegmentBuf::Field(FieldBuf { name, .. }) => handle_field(name, new, segments),

            SegmentBuf::Coalesce(fields) => {
                // At this point, we know that the coalesced field query did not
                // result in an actual value, so none of the fields match an
                // existing field. We'll pick the last field in the list to
                // insert the new value into.
                let Some(field) = fields.last() else {return};

                handle_field(field.as_str(), new, segments);
            }
            SegmentBuf::Index(index) => {
                let array = match self {
                    Self::Array(array) => array,
                    _ => {
                        *self = Self::Array(vec![]);
                        self.as_array_mut().unwrap()
                    }
                };

                let index = *index;

                // If we're dealing with a negative index, we either need to
                // replace an existing value, or insert to the front of the
                // array.
                if index.is_negative() {
                    let abs = index.unsigned_abs();

                    // left-padded with null values
                    for _ in 1..abs - array.len() {
                        array.insert(0, Self::Null);
                    }

                    match segments.peek() {
                        None => {
                            array.insert(0, new);
                            return;
                        }
                        Some(next) => match next {
                            SegmentBuf::Index(_) => array.insert(0, Self::Array(vec![])),
                            _ => array.insert(0, BTreeMap::default().into()),
                        },
                    };

                    array
                        .first_mut()
                        .expect("exists")
                        .insert_by_segments(segments, new);
                } else {
                    let index = index as usize;

                    // right-padded with null values
                    if array.len() < index {
                        array.resize(index, Self::Null);
                    }

                    match segments.peek() {
                        None => {
                            array.push(new);
                            return;
                        }
                        Some(next) => match next {
                            SegmentBuf::Index(_) => array.push(Self::Array(vec![])),
                            _ => array.push(BTreeMap::default().into()),
                        },
                    }

                    array
                        .last_mut()
                        .expect("exists")
                        .insert_by_segments(segments, new);
                }
            }
        }
    }
}
