use super::ValueCollection;
use crate::path::BorrowedSegment;
use crate::value::Value;
use std::borrow::Borrow;
use std::collections::BTreeMap;

pub fn insert<'a, T: ValueCollection>(
    value: &mut T,
    // Traversing an existing ancestor only needs a borrowed key. Convert to an
    // owned collection key only when storing a leaf or replacing a container.
    key: impl Borrow<T::BorrowedKey> + Into<T::Key>,
    mut path_iter: impl Iterator<Item = BorrowedSegment<'a>>,
    insert_value: Value,
) -> Option<Value> {
    match path_iter.next() {
        Some(BorrowedSegment::Field(field)) => {
            if let Some(Value::Object(map)) = value.get_mut_value(key.borrow()) {
                insert(map, field, path_iter, insert_value)
            } else {
                let mut map = BTreeMap::new();
                let prev_value = insert(&mut map, field, path_iter, insert_value);
                value.insert_value(key.into(), Value::Object(map));
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
                value.insert_value(key.into(), Value::Array(array));
                prev_value
            }
        }
        Some(BorrowedSegment::Invalid) => None,
        None => value.insert_value(key.into(), insert_value),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::path;
    use serde_json::json;

    #[test]
    fn test_insert_new_leaf_existing_prefix() {
        let mut value = Value::from(json!({
            "kubernetes": {"pod_labels": {"app": "web"}},
            "message": "hello"
        }));
        assert_eq!(
            value.insert(path!("kubernetes", "pod_labels", "team"), "infra"),
            None
        );
        assert_eq!(
            value.insert(path!("kubernetes", "pod_labels", "app"), "api"),
            Some(Value::from("web"))
        );
        assert_eq!(
            value,
            Value::from(json!({
                "kubernetes": {"pod_labels": {"app": "api", "team": "infra"}},
                "message": "hello"
            }))
        );
    }

    #[test]
    fn test_insert_owned_field_segments() {
        let mut value = Value::from(json!({"a": {"sibling": 1}}));
        let segments = ["a", "b"].map(|field| BorrowedSegment::Field(field.to_owned().into()));
        assert_eq!(
            insert(&mut value, (), segments.into_iter(), Value::Integer(2)),
            None
        );
        assert_eq!(value, Value::from(json!({"a": {"sibling": 1, "b": 2}})));
    }

    #[test]
    fn test_insert_empty_path_replaces_root() {
        for original in [json!(null), json!(1), json!({"a": 1}), json!([1, 2])] {
            for replacement in [json!(42), json!({"b": 2}), json!([3])] {
                let mut value = Value::from(original.clone());
                let replacement = Value::from(replacement);
                assert_eq!(
                    value.insert(path!(), replacement.clone()),
                    Some(Value::from(original.clone()))
                );
                assert_eq!(value, replacement);
            }
        }
    }

    #[test]
    fn test_insert_invalid_segments_preserves_partial_mutations() {
        use BorrowedSegment::{Field, Index, Invalid};

        let cases = [
            ("invalid root", json!(1), vec![Invalid], json!(1)),
            (
                "existing prefix",
                json!({"a": {"keep": 1}}),
                vec![Field("a".into()), Invalid],
                json!({"a": {"keep": 1}}),
            ),
            (
                "missing prefix",
                json!({"keep": 1}),
                vec![Field("a".into()), Invalid],
                json!({"keep": 1}),
            ),
            (
                "missing object root",
                json!(null),
                vec![Field("a".into()), Invalid],
                json!({}),
            ),
            (
                "replace scalar with object",
                json!({"a": 1, "keep": 2}),
                vec![Field("a".into()), Field("b".into()), Invalid],
                json!({"a": {}, "keep": 2}),
            ),
            (
                "existing array prefix",
                json!({"a": [1], "keep": 2}),
                vec![Field("a".into()), Index(0), Invalid],
                json!({"a": [1], "keep": 2}),
            ),
            (
                "replace scalar with array",
                json!({"a": 1, "keep": 2}),
                vec![Field("a".into()), Index(0), Invalid],
                json!({"a": [], "keep": 2}),
            ),
            (
                "missing array root",
                json!(null),
                vec![Index(0), Invalid],
                json!([]),
            ),
            (
                "missing mixed containers",
                json!(null),
                vec![Index(0), Field("a".into()), Invalid],
                json!([{}]),
            ),
        ];

        for (name, original, segments, expected) in cases {
            let mut value = Value::from(original);
            assert_eq!(value.insert(&segments, 42), None, "{name}: return value");
            assert_eq!(value, Value::from(expected), "{name}: final value");
        }
    }

    #[test]
    fn test_insert_container_replacement_returns_leaf_value() {
        let cases = [
            (
                json!({"a": 1}),
                vec!["a".into(), "b".into()],
                json!({"a": {"b": 42}}),
                None,
            ),
            (
                json!({"a": [1]}),
                vec!["a".into(), "b".into()],
                json!({"a": {"b": 42}}),
                None,
            ),
            (
                json!({"a": 1}),
                vec!["a".into(), 1_isize.into()],
                json!({"a": [null, 42]}),
                None,
            ),
            (
                json!({"a": {"b": 1}}),
                vec!["a".into(), 0_isize.into()],
                json!({"a": [42]}),
                None,
            ),
            (
                json!({"a": {"b": 1}}),
                vec!["a".into()],
                json!({"a": 42}),
                Some(json!({"b": 1})),
            ),
            (
                json!({"a": [1]}),
                vec!["a".into()],
                json!({"a": 42}),
                Some(json!([1])),
            ),
            (
                json!({"a": [1]}),
                vec!["a".into(), 0_isize.into()],
                json!({"a": [42]}),
                Some(json!(1)),
            ),
        ];

        for (original, segments, expected, previous) in cases {
            let mut value = Value::from(original);
            let segments: Vec<BorrowedSegment<'_>> = segments;
            assert_eq!(value.insert(&segments, 42), previous.map(Value::from));
            assert_eq!(value, Value::from(expected));
        }
    }

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
}
