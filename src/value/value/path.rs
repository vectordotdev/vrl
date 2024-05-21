use crate::path::ValuePath;

use crate::value::Value;

impl Value {
    /// Insert the current value into a given path.
    ///
    /// For example, given the path `.foo.bar` and value `true`, the return
    /// value would be an object representing `{ "foo": { "bar": true } }`.
    #[must_use]
    pub fn at_path<'a>(self, path: impl ValuePath<'a>) -> Self {
        let mut result = Self::Null;
        result.insert(path, self);
        result
    }
}

#[cfg(test)]
mod at_path_tests {
    use std::collections::BTreeMap;

    use crate::path;
    use crate::path::parse_value_path;

    use crate::value::Value;

    #[test]
    fn test_object() {
        let path = parse_value_path(".foo.bar.baz").unwrap();
        let value = Value::Integer(12);

        let bar_value = Value::Object(BTreeMap::from([("baz".into(), value.clone())]));
        let foo_value = Value::Object(BTreeMap::from([("bar".into(), bar_value)]));

        let object = Value::Object(BTreeMap::from([("foo".into(), foo_value)]));

        assert_eq!(value.at_path(&path), object);
    }

    #[test]
    fn test_root() {
        let value = Value::Integer(12);

        let object = Value::Integer(12);

        assert_eq!(value.at_path(path!()), object);
    }

    #[test]
    fn test_array() {
        let path = parse_value_path("[2]").unwrap();
        let value = Value::Integer(12);

        let object = Value::Array(vec![Value::Null, Value::Null, Value::Integer(12)]);

        assert_eq!(value.at_path(&path), object);
    }

    #[test]
    fn test_complex() {
        let path = parse_value_path("[2].foo.baz[1]").unwrap();
        let value = Value::Object([("bar".into(), vec![12].into())].into()); //value!({ "bar": [12] });

        let baz_value = Value::Array(vec![Value::Null, value.clone()]);
        let foo_value = Value::Object(BTreeMap::from([("baz".into(), baz_value)]));

        let object = Value::Array(vec![
            Value::Null,
            Value::Null,
            Value::Object(BTreeMap::from([("foo".into(), foo_value)])),
        ]);

        assert_eq!(value.at_path(&path), object);
    }
}
