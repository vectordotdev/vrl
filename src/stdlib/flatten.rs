use std::collections::btree_map;

use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_SEPARATOR: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from(".")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The array or object to flatten.",
            default: None,
        },
        Parameter {
            keyword: "separator",
            kind: kind::BYTES,
            required: false,
            description: "The separator to join nested keys",
            default: Some(&DEFAULT_SEPARATOR),
        },
    ]
});

fn flatten(value: Value, separator: &Value) -> Resolved {
    let separator = separator.try_bytes_utf8_lossy()?;

    match value {
        Value::Array(arr) => Ok(Value::Array(
            ArrayFlatten::new(arr.iter()).cloned().collect(),
        )),
        Value::Object(map) => Ok(Value::Object(
            MapFlatten::new(map.iter(), &separator)
                .map(|(k, v)| (k, v.clone()))
                .collect(),
        )),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::array(Collection::any()) | Kind::object(Collection::any()),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Flatten;

impl Function for Flatten {
    fn identifier(&self) -> &'static str {
        "flatten"
    }

    fn usage(&self) -> &'static str {
        "Flattens the `value` into a single-level representation."
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Flatten array",
                source: "flatten([1, [2, 3, 4], [5, [6, 7], 8], 9])",
                result: Ok("[1, 2, 3, 4, 5, 6, 7, 8, 9]"),
            },
            example! {
                title: "Flatten object",
                source: r#"flatten({ "parent1": { "child1": 1, "child2": 2 }, "parent2": { "child3": 3 } })"#,
                result: Ok(r#"{ "parent1.child1": 1, "parent1.child2": 2, "parent2.child3": 3 }"#),
            },
            example! {
                title: "Flatten object with custom separator",
                source: r#"flatten({ "foo": { "bar": true }}, "_")"#,
                result: Ok(r#"{ "foo_bar": true }"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let separator = arguments.optional("separator");
        let value = arguments.required("value");
        Ok(FlattenFn { value, separator }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct FlattenFn {
    value: Box<dyn Expression>,
    separator: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FlattenFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let separator = self
            .separator
            .map_resolve_with_default(ctx, || DEFAULT_SEPARATOR.clone())?;

        flatten(value, &separator)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        if td.is_array() {
            TypeDef::array(Collection::any())
        } else {
            TypeDef::object(Collection::any())
        }
    }
}

/// An iterator to walk over maps allowing us to flatten nested maps to a single level.
struct MapFlatten<'a> {
    values: btree_map::Iter<'a, KeyString, Value>,
    separator: &'a str,
    inner: Option<Box<MapFlatten<'a>>>,
    parent: Option<KeyString>,
}

impl<'a> MapFlatten<'a> {
    fn new(values: btree_map::Iter<'a, KeyString, Value>, separator: &'a str) -> Self {
        Self {
            values,
            separator,
            inner: None,
            parent: None,
        }
    }

    fn new_from_parent(
        parent: KeyString,
        values: btree_map::Iter<'a, KeyString, Value>,
        separator: &'a str,
    ) -> Self {
        Self {
            values,
            separator,
            inner: None,
            parent: Some(parent),
        }
    }

    /// Returns the key with the parent prepended.
    fn new_key(&self, key: &str) -> KeyString {
        match self.parent {
            None => key.to_string().into(),
            Some(ref parent) => format!("{parent}{}{key}", self.separator).into(),
        }
    }
}

impl<'a> std::iter::Iterator for MapFlatten<'a> {
    type Item = (KeyString, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref mut inner) = self.inner {
            let next = inner.next();
            match next {
                Some(_) => return next,
                None => self.inner = None,
            }
        }

        let next = self.values.next();
        match next {
            Some((key, Value::Object(value))) => {
                self.inner = Some(Box::new(MapFlatten::new_from_parent(
                    self.new_key(key),
                    value.iter(),
                    self.separator,
                )));
                self.next()
            }
            Some((key, value)) => Some((self.new_key(key), value)),
            None => None,
        }
    }
}

/// Create an iterator that can walk a tree of Array values.
/// This can be used to flatten the array.
struct ArrayFlatten<'a> {
    values: std::slice::Iter<'a, Value>,
    inner: Option<Box<ArrayFlatten<'a>>>,
}

impl<'a> ArrayFlatten<'a> {
    fn new(values: std::slice::Iter<'a, Value>) -> Self {
        ArrayFlatten {
            values,
            inner: None,
        }
    }
}

impl<'a> std::iter::Iterator for ArrayFlatten<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate over our inner list first.
        if let Some(ref mut inner) = self.inner {
            let next = inner.next();
            match next {
                Some(_) => return next,
                None => {
                    // The inner list has been exhausted.
                    self.inner = None;
                }
            }
        }

        // Then iterate over our values.
        let next = self.values.next();
        match next {
            Some(Value::Array(next)) => {
                // Create a new iterator for this child list.
                self.inner = Some(Box::new(ArrayFlatten::new(next.iter())));
                self.next()
            }
            _ => next,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        flatten => Flatten;

        array {
            args: func_args![value: value!([42])],
            want: Ok(value!([42])),
            tdef: TypeDef::array(Collection::any()),
        }

        nested_array {
            args: func_args![value: value!([42, [43, 44]])],
            want: Ok(value!([42, 43, 44])),
            tdef: TypeDef::array(Collection::any()),
        }

        nested_empty_array {
            args: func_args![value: value!([42, [], 43])],
            want: Ok(value!([42, 43])),
            tdef: TypeDef::array(Collection::any()),
        }

        double_nested_array {
            args: func_args![value: value!([42, [43, 44, [45, 46]]])],
            want: Ok(value!([42, 43, 44, 45, 46])),
            tdef: TypeDef::array(Collection::any()),
        }

        two_arrays {
            args: func_args![value: value!([[42, 43], [44, 45]])],
            want: Ok(value!([42, 43, 44, 45])),
            tdef: TypeDef::array(Collection::any()),
        }

        map {
            args: func_args![value: value!({parent: "child"})],
            want: Ok(value!({parent: "child"})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map {
            args: func_args![value: value!({parent: {child1: 1, child2: 2}, key: "val"})],
            want: Ok(value!({"parent.child1": 1, "parent.child2": 2, key: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map_with_separator {
            args: func_args![value: value!({parent: {child1: 1, child2: 2}, key: "val"}), separator: "_"],
            want: Ok(value!({"parent_child1": 1, "parent_child2": 2, key: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        double_nested_map {
            args: func_args![value: value!({
                parent: {
                    child1: 1,
                    child2: { grandchild1: 1, grandchild2: 2 },
                },
                key: "val",
            })],
            want: Ok(value!({
                "parent.child1": 1,
                "parent.child2.grandchild1": 1,
                "parent.child2.grandchild2": 2,
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        map_and_array {
            args: func_args![value: value!({
                parent: {
                    child1: [1, [2, 3]],
                    child2: {grandchild1: 1, grandchild2: [1, [2, 3], 4]},
                },
                key: "val",
            })],
            want: Ok(value!({
                "parent.child1": [1, [2, 3]],
                "parent.child2.grandchild1": 1,
                "parent.child2.grandchild2": [1, [2, 3], 4],
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        map_and_array_with_separator {
            args: func_args![value: value!({
                parent: {
                    child1: [1, [2, 3]],
                    child2: {grandchild1: 1, grandchild2: [1, [2, 3], 4]},
                },
                key: "val",
            }), separator: "_"],
            want: Ok(value!({
                "parent_child1": [1, [2, 3]],
                "parent_child2_grandchild1": 1,
                "parent_child2_grandchild2": [1, [2, 3], 4],
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        // If the root object is an array, child maps are not flattened.
        root_array {
            args: func_args![value: value!([
                { parent1: { child1: 1, child2: 2 } },
                [
                    { parent2: { child3: 3, child4: 4 } },
                    { parent3: { child5: 5 } },
                ],
            ])],
            want: Ok(value!([
                { parent1: { child1: 1, child2: 2 } },
                { parent2: { child3: 3, child4: 4 } },
                { parent3: { child5: 5 } },
            ])),
            tdef: TypeDef::array(Collection::any()),
        }

        triple_nested_map {
            args: func_args![value: value!({
                parent1: {
                    child1: { grandchild1: 1 },
                    child2: { grandchild2: 2, grandchild3: 3 },
                },
                parent2: 4,
            })],
            want: Ok(value!({
                "parent1.child1.grandchild1": 1,
                "parent1.child2.grandchild2": 2,
                "parent1.child2.grandchild3": 3,
                parent2: 4,
            })),
            tdef: TypeDef::object(Collection::any()),
        }
    ];
}
