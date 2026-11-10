use super::{ValueCollection, ValueSlot};
use crate::path::BorrowedSegment;
use crate::value::Value;
use std::borrow::Borrow;
use std::collections::BTreeMap;

pub fn insert<'a, T: ValueCollection>(
    value: &mut T,
    key: T::Key,
    path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    insert_value: Value,
) -> Option<Value> {
    insert_with_hook(value, key, path_iter, insert_value, no_hook)
}

/// Same as [`insert`], calling `hook` right before the single modification of the existing tree.
pub fn insert_with_hook<'a, T: ValueCollection>(
    value: &mut T,
    key: T::Key,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    insert_value: Value,
    hook: impl FnOnce(ValueSlot<'_>, &Value),
) -> Option<Value> {
    match path_iter.next() {
        Some(BorrowedSegment::Field(field)) => {
            if let Some(Value::Object(map)) = value.get_mut_value(key.borrow()) {
                insert_with_hook(map, field.into(), path_iter, insert_value, hook)
            } else {
                let mut map = BTreeMap::new();
                let prev_value = insert(&mut map, field.into(), path_iter, insert_value);
                insert_value_with_hook(value, key, Value::Object(map), hook);
                prev_value
            }
        }
        Some(BorrowedSegment::Index(index)) => {
            if let Some(Value::Array(array)) = value.get_mut_value(key.borrow()) {
                insert_with_hook(array, index, path_iter, insert_value, hook)
            } else {
                let capacity = if index >= 0 {
                    (index as usize) + 1
                } else {
                    (-index) as usize
                };
                let mut array = Vec::with_capacity(capacity);
                let prev_value = insert(&mut array, index, path_iter, insert_value);
                insert_value_with_hook(value, key, Value::Array(array), hook);
                prev_value
            }
        }
        Some(BorrowedSegment::Invalid) => None,
        None => insert_value_with_hook(value, key, insert_value, hook),
    }
}

fn insert_value_with_hook<T: ValueCollection>(
    collection: &mut T,
    key: T::Key,
    value: Value,
    hook: impl FnOnce(ValueSlot<'_>, &Value),
) -> Option<Value> {
    hook(collection.slot(key.borrow()), &value);
    collection.insert_value(key, value)
}

fn no_hook(_: ValueSlot<'_>, _: &Value) {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::path;
    use serde_json::json;

    #[test]
    fn test_insert_nested() {
        let mut value = Value::Null;
        value.insert(path!("a", "b", "c"), 3);
        let expected = Value::from(json!({
            "a": {
                "b":{
                    "c": 3
                }
            }
        }));
        assert_eq!(value, expected);
    }

    #[test]
    fn test_insert_array() {
        let mut value = Value::Null;
        value.insert(path!("a", "b", 0_isize, "c", 2_isize), 10);
        value.insert(path!("a", "b", 0_isize, "c", 0_isize), 5);

        let expected = Value::from(json!({
            "a": {
                "b": [{
                    "c": [5, null, 10]
                }]
            }
        }));
        assert_eq!(value, expected);
    }

    #[test]
    fn test_insert_negative_index() {
        let mut value = Value::Null;
        assert_eq!(value.insert(path!(-2_isize), 10), None);
        assert_eq!(value.insert(path!(-1_isize), 5), Some(Value::Null));
        assert_eq!(value.insert(path!(-2_isize), 2), Some(Value::Integer(10)));
        assert_eq!(value.insert(path!(-1_isize, 1_isize), 3), None);
        assert_eq!(value, Value::from(json!([2, [null, 3]])));
    }

    #[test]
    fn test_insert_with_hook() {
        use crate::path::parse_value_path;
        use crate::value::value::crud::owned_slot;

        let cases = [
            // Existing parent object.
            (
                json!({"a": {"b": 1}}),
                "a.c",
                (json!({"b": 1}), "c"),
                json!(2),
            ),
            // Missing parents, replacing a non-container.
            (
                json!({"a": "s"}),
                "a.b.c",
                (json!({"a": "s"}), "a"),
                json!({"b": {"c": 2}}),
            ),
            // Array padding.
            (json!({"a": [1]}), "a[2]", (json!([1]), "2"), json!(2)),
            // Non-container root.
            (json!(null), "a", (json!(null), ""), json!({"a": 2})),
            // Root path.
            (json!({"a": 1}), ".", (json!({"a": 1}), ""), json!(2)),
        ];

        for (value, path, (parent, key), written) in cases {
            let mut value = Value::from(value);
            let path = parse_value_path(path).unwrap();
            let mut expected = value.clone();
            let expected_prev = expected.insert(&path, 2);

            let mut calls = vec![];
            let prev = value.insert_with_hook(&path, 2, |slot, inserted| {
                calls.push((owned_slot(slot), inserted.clone()));
            });

            assert_eq!(
                calls,
                [((Value::from(parent), key.to_owned()), Value::from(written))],
                "{path}"
            );
            assert_eq!((value, prev), (expected, expected_prev), "{path}");
        }
    }
}
