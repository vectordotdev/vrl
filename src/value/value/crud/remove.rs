use super::ValueCollection;
use crate::path::BorrowedSegment;
use crate::value::Value;

pub fn remove<'a, T: ValueCollection>(
    value: &mut T,
    key: &T::BorrowedKey,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    prune: bool,
) -> Option<(Value, bool)> {
    match path_iter.next() {
        None => value.remove_value(key),
        Some(next_segment) => match (value.get_mut_value(key), next_segment) {
            (Some(Value::Object(map)), BorrowedSegment::Field(field)) => {
                let (prev_value, empty) = remove(map, field.as_ref(), path_iter, prune)?;
                if prune && empty {
                    value.remove_value(key);
                }
                Some(prev_value)
            }
            (Some(Value::Array(array)), BorrowedSegment::Index(index)) => {
                let (prev_value, empty) = remove(array, &index, path_iter, prune)?;
                if prune && empty {
                    value.remove_value(key);
                }
                Some(prev_value)
            }
            _ => return None,
        },
    }
    .map(|prev| (prev, value.is_empty_collection()))
}

#[cfg(test)]
mod test {
    use crate::path;
    use crate::path::parse_value_path;
    use crate::value::Value;
    use serde_json::json;

    #[test]
    fn array_remove_from_middle() {
        let mut value = Value::Array(vec![Value::Null, Value::Integer(3)]);
        assert_eq!(value.remove(path!(0_isize), false), Some(Value::Null));
        assert_eq!(value.remove(path!(0_isize), false), Some(Value::Integer(3)));
        assert_eq!(value.remove(path!(0_isize), false), None);
    }

    #[test]
    fn remove_simple() {
        let mut value = Value::from(json!({
            "field": 123
        }));
        assert_eq!(
            value.remove(path!("field"), false),
            Some(Value::Integer(123))
        );
        assert_eq!(value.remove(path!("field"), false), None);
    }

    #[test]
    fn remove_nested() {
        let mut value = Value::from(json!({
            "a": {
                "b": {
                    "c": 5
                },
                "d": 4,
                "array": [null, 3, {
                    "x": 1
                }, [2]]
            }
        }));
        let queries = [
            ("a.b.c", Some(Value::Integer(5)), None),
            ("a.d", Some(Value::Integer(4)), None),
            ("a.array[2].x", Some(Value::Integer(1)), None),
            ("a.array[3][0]", Some(Value::Integer(2)), None),
            ("a.array[3][1]", None, None),
            ("a.x", None, None),
            ("z", None, None),
            (".123", None, None),
        ];

        for (query, expected_first, expected_second) in &queries {
            let parsed = parse_value_path(query).unwrap();
            assert_eq!(value.remove(&parsed, false), *expected_first, "{query}");
            assert_eq!(value.remove(&parsed, false), *expected_second, "{query}");
        }

        assert_eq!(
            value,
            Value::from(json!({
                "a": {
                    "b": {},
                    "array": [
                        null,
                        3,
                        {},
                        [],
                    ],
                },
            }))
        );

        value.remove(&parse_value_path(".").unwrap(), false);
        assert_eq!(value, Value::from(json!({})));
        value.remove(&parse_value_path(".").unwrap(), true);
        assert_eq!(value, Value::from(json!({})));
    }

    #[test]
    fn remove_prune() {
        let mut value = Value::from(json!({
            "a": {
                "b": {
                    "c": vec![5]
                },
                "d": 4,
            }
        }));

        assert_eq!(value.remove(path!("a", "d"), true), Some(Value::Integer(4)));
        assert_eq!(
            value,
            Value::from(json!({
                "a": {
                    "b": {
                        "c": vec![5]
                    }
                }
            }))
        );

        assert_eq!(
            value.remove(path!("a", "b", "c", 0_isize), true),
            Some(Value::Integer(5))
        );
        assert_eq!(value, Value::from(json!({})));
    }

    #[test]
    fn remove_leaf_does_not_call_get_mut_value() {
        use super::super::ValueCollection;
        use super::remove;
        use crate::path::{BorrowedSegment, ValuePath};
        use crate::value::{KeyString, ObjectMap};

        struct CountingMap {
            inner: ObjectMap,
            get_mut_calls: usize,
            remove_calls: usize,
        }

        impl ValueCollection for CountingMap {
            type Key = KeyString;
            type BorrowedKey = str;

            fn get_value(&self, key: &str) -> Option<&Value> {
                self.inner.get(key)
            }

            fn get_mut_value(&mut self, key: &str) -> Option<&mut Value> {
                self.get_mut_calls += 1;
                self.inner.get_mut(key)
            }

            fn insert_value(&mut self, key: KeyString, value: Value) -> Option<Value> {
                self.inner.insert(key, value)
            }

            fn remove_value(&mut self, key: &str) -> Option<Value> {
                self.remove_calls += 1;
                self.inner.remove(key)
            }

            fn is_empty_collection(&self) -> bool {
                self.inner.is_empty()
            }
        }

        // Leaf removal should call remove_value directly without calling get_mut_value.
        let mut leaf_map = CountingMap {
            inner: ObjectMap::from([("leaf".into(), Value::Integer(42))]),
            get_mut_calls: 0,
            remove_calls: 0,
        };
        let res = remove(
            &mut leaf_map,
            "leaf",
            std::iter::empty::<BorrowedSegment<'_>>(),
            false,
        );
        assert_eq!(res, Some((Value::Integer(42), true)));
        assert_eq!(leaf_map.get_mut_calls, 0);
        assert_eq!(leaf_map.remove_calls, 1);

        // Nested removal should call get_mut_value only for the intermediate segment.
        let mut nested_map = CountingMap {
            inner: ObjectMap::from([("nested".into(), Value::from(json!({ "leaf": 7 })))]),
            get_mut_calls: 0,
            remove_calls: 0,
        };
        let res = remove(
            &mut nested_map,
            "nested",
            path!("leaf").segment_iter(),
            false,
        );
        assert_eq!(res, Some((Value::Integer(7), false)));
        assert_eq!(nested_map.get_mut_calls, 1);
        assert_eq!(nested_map.remove_calls, 0);
    }
}
