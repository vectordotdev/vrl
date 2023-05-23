use super::{get_matching_coalesce_key, skip_remaining_coalesce_segments, ValueCollection};
use crate::path::BorrowedSegment;
use crate::value::Value;
use std::borrow::Borrow;
use std::collections::BTreeMap;

pub fn insert<'a, T: ValueCollection>(
    value: &mut T,
    key: T::Key,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    insert_value: Value,
) -> Option<Value> {
    match path_iter.next() {
        Some(BorrowedSegment::Field(field)) => {
            if let Some(Value::Object(map)) = value.get_mut_value(key.borrow()) {
                insert(map, field.to_string(), path_iter, insert_value)
            } else {
                let mut map = BTreeMap::new();
                let prev_value = insert(&mut map, field.to_string(), path_iter, insert_value);
                value.insert_value(key, Value::Object(map));
                prev_value
            }
        }
        Some(BorrowedSegment::Index(index)) => {
            if let Some(Value::Array(array)) = value.get_mut_value(key.borrow()) {
                insert(array, index, path_iter, insert_value)
            } else {
                let capacity = if index >= 0 {
                    (index as usize) + 1
                } else {
                    (-index) as usize
                };
                let mut array = Vec::with_capacity(capacity);
                let prev_value = insert(&mut array, index, path_iter, insert_value);
                value.insert_value(key, Value::Array(array));
                prev_value
            }
        }
        Some(BorrowedSegment::CoalesceField(field)) => {
            if let Some(Value::Object(map)) = value.get_mut_value(key.borrow()) {
                let (Ok(matched_key) | Err(matched_key)) =
                    get_matching_coalesce_key(field, map, &mut path_iter);
                insert(map, matched_key.to_string(), path_iter, insert_value)
            } else {
                let mut map = BTreeMap::new();
                // The map is empty, so only the last segment will be used.
                let last_coalesce_key = skip_remaining_coalesce_segments(&mut path_iter);
                let prev_value = insert(
                    &mut map,
                    last_coalesce_key.to_string(),
                    path_iter,
                    insert_value,
                );
                value.insert_value(key, Value::Object(map));
                prev_value
            }
        }
        Some(BorrowedSegment::CoalesceEnd(_)) => unreachable!("malformed path. This is a bug."),
        Some(BorrowedSegment::Invalid) => None,
        None => value.insert_value(key, insert_value),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_insert_coalesce() {
        let mut value = Value::Null;

        value.insert("(a|b)", 1);
        assert_eq!(
            value,
            Value::from(json!({
                "b": 1
            }))
        );

        value.insert("(a|b|c)", 2);
        assert_eq!(
            value,
            Value::from(json!({
                "b": 2
            }))
        );

        value.insert("(a|b|c)", Value::from(json!({"x": true})));
        assert_eq!(
            value,
            Value::from(json!({
                "b": {"x": true}
            }))
        );

        value.insert("(c|a)", 13);
        assert_eq!(
            value,
            Value::from(json!({
                "a": 13,
                "b": {"x": true}
            }))
        );

        value.insert("(a|b).x", 5);
        assert_eq!(
            value,
            Value::from(json!({
                "a": {"x": 5},
                "b": {"x": true}
            }))
        );
    }

    #[test]
    fn test_insert_nested() {
        let mut value = Value::Null;
        value.insert("a.b.c", 3);
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
        value.insert("a.b[0].c[2]", 10);
        value.insert("a.b[0].c[0]", 5);

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
        assert_eq!(value.insert("[-2]", 10), None);
        assert_eq!(value.insert("[-1]", 5), Some(Value::Null));
        assert_eq!(value.insert("[-2]", 2), Some(Value::Integer(10)));
        assert_eq!(value.insert("[-1][1]", 3), None);
        assert_eq!(value, Value::from(json!([2, [null, 3]])));
    }
}
