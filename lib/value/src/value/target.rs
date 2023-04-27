use std::iter::Peekable;

use lookup::{FieldBuf, LookupBuf, SegmentBuf};

use crate::Value;

impl Value {
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
}
