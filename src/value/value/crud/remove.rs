use super::{ValueCollection, ValueSlot};
use crate::path::BorrowedSegment;
use crate::value::Value;

pub fn remove<'a, T: ValueCollection>(
    value: &mut T,
    key: &T::BorrowedKey,
    path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    prune: bool,
) -> Option<(Value, bool)> {
    remove_with_hook(value, key, path_iter, prune, &mut no_hook)
}

/// Same as [`remove`], calling `hook` right before each removal from the existing tree.
pub fn remove_with_hook<'a, T: ValueCollection>(
    value: &mut T,
    key: &T::BorrowedKey,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    prune: bool,
    hook: &mut impl FnMut(ValueSlot<'_>),
) -> Option<(Value, bool)> {
    match (value.get_mut_value(key), path_iter.next()) {
        (current, None) => {
            if current.is_some() {
                hook(value.slot(key));
            }
            value.remove_value(key)
        }
        (Some(Value::Object(map)), Some(BorrowedSegment::Field(field))) => {
            let (prev_value, empty) =
                remove_with_hook(map, field.as_ref(), path_iter, prune, hook)?;
            if prune && empty {
                hook(value.slot(key));
                value.remove_value(key);
            }
            Some(prev_value)
        }
        (Some(Value::Array(array)), Some(BorrowedSegment::Index(index))) => {
            let (prev_value, empty) = remove_with_hook(array, &index, path_iter, prune, hook)?;
            if prune && empty {
                hook(value.slot(key));
                value.remove_value(key);
            }
            Some(prev_value)
        }
        _ => return None,
    }
    .map(|prev| (prev, value.is_empty_collection()))
}

fn no_hook(_: ValueSlot<'_>) {}

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
    fn remove_with_hook() {
        use crate::value::value::crud::owned_slot;

        let value = json!({"a": {"b": 1}, "c": [1, 2]});
        let cases = [
            ("a.b", false, vec![(json!({"b": 1}), "b")]),
            (
                "a.b",
                true,
                vec![(json!({"b": 1}), "b"), (json!({"a": {}, "c": [1, 2]}), "a")],
            ),
            ("c[-1]", false, vec![(json!([1, 2]), "-1")]),
            ("missing", false, vec![]),
            ("a.b.c", false, vec![]),
        ];

        for (path, prune, expected_calls) in cases {
            let mut value = Value::from(value.clone());
            let path = parse_value_path(path).unwrap();
            let mut expected = value.clone();
            let expected_prev = expected.remove(&path, prune);

            let mut calls = vec![];
            let prev = value.remove_with_hook(&path, prune, |slot| calls.push(owned_slot(slot)));

            let expected_calls = expected_calls
                .into_iter()
                .map(|(parent, key)| (Value::from(parent), key.to_owned()))
                .collect::<Vec<_>>();
            assert_eq!(calls, expected_calls, "{path}");
            assert_eq!((value, prev), (expected, expected_prev), "{path}");
        }
    }
}
