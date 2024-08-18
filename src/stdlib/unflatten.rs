use itertools::Itertools;

use crate::compiler::prelude::*;

static DEFAULT_SEPARATOR: &str = ".";

fn unflatten(value: Value, separator: Value) -> Resolved {
    let separator = separator.try_bytes_utf8_lossy()?.into_owned();
    let map = value.try_object()?;
    Ok(do_unflatten(map.into(), &separator))
}

fn do_unflatten(value: Value, separator: &str) -> Value {
    match value {
        Value::Object(map) => do_unflatten_entries(map, separator).into(),
        // Note that objects inside arrays are not unflattened
        _ => value,
    }
}

// this should return the key to insert?
fn do_unflatten_entries<I>(entries: I, separator: &str) -> ObjectMap
where
    I: IntoIterator<Item = (KeyString, Value)>,
{
    let grouped = entries
        .into_iter()
        .map(|(key, value)| {
            let (head, rest) = match key.split_once(separator) {
                Some((key, rest)) => (key.to_string().into(), Some(rest.to_string())),
                None => (key.clone(), None),
            };
            (head, rest, value)
        })
        .into_group_map_by(|(head, _, _)| head.clone());

    grouped
        .into_iter()
        .map(|(key, mut values)| {
            if values.len() == 1 {
                match values.pop().unwrap() {
                    (_, None, value) => return (key, do_unflatten(value, separator)),
                    (_, Some(rest), value) => {
                        let result = do_unflatten_entries([(rest.into(), value)], separator);
                        return (key, result.into());
                    }
                }
            }

            let new_entries = values
                .into_iter()
                .filter_map(|(_, rest, value)| {
                    // In this case, there is more than one value with the same key
                    // and then there must be nested values, we can't set a single top-level value
                    // so we filter it out.
                    // Example input of this case:
                    // {
                    //    "a.b": 1,
                    //    "a": 2
                    // }
                    // Here, we will have two items grouped by "a",
                    // one will have "b" as rest and the other will have None.
                    // We have to filter the second, as we can't set the second value
                    // as the value of "a" (considered the top-level key at this level)
                    rest.map(|rest| (rest.into(), value))
                })
                .collect::<Vec<_>>();
            let result = do_unflatten_entries(new_entries, separator);
            (key, result.into())
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Unflatten;

impl Function for Unflatten {
    fn identifier(&self) -> &'static str {
        "unflatten"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT,
                required: true,
            },
            Parameter {
                keyword: "separator",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "object",
                source: r#"{ "foo.bar": true }"#,
                result: Ok(r#"flatten({ "foo": { "bar": true }})"#),
            },
            Example {
                title: "object",
                source: r#"{ "foo_bar": true }"#,
                result: Ok(r#"flatten({ "foo": { "bar": true }}, "_")"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let separator = arguments
            .optional("separator")
            .unwrap_or_else(|| expr!(DEFAULT_SEPARATOR));
        let value = arguments.required("value");
        Ok(UnflattenFn { value, separator }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UnflattenFn {
    value: Box<dyn Expression>,
    separator: Box<dyn Expression>,
}

impl FunctionExpression for UnflattenFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let separator = self.separator.resolve(ctx)?;

        unflatten(value, separator)
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::object(Collection::any())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        unflatten => Unflatten;

        map {
            args: func_args![value: value!({parent: "child"})],
            want: Ok(value!({parent: "child"})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map {
            args: func_args![value: value!({"parent.child1": 1, "parent.child2": 2, key: "val"})],
            want: Ok(value!({parent: {child1: 1, child2: 2}, key: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map_with_separator {
            args: func_args![value: value!({"parent_child1": 1, "parent_child2": 2, key: "val"}), separator: "_"],
            want: Ok(value!({parent: {child1: 1, child2: 2}, key: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        double_nested_map {
            args: func_args![value: value!({
                "parent.child1": 1,
                "parent.child2.grandchild1": 1,
                "parent.child2.grandchild2": 2,
                key: "val",
            })],
            want: Ok(value!({
                parent: {
                    child1: 1,
                    child2: { grandchild1: 1, grandchild2: 2 },
                },
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        // Not only keys at first level are unflattened
        double_inner_nested_map {
            args: func_args![value: value!({
                "parent": {"child1":1, "child2.grandchild1": 1, "child2.grandchild2": 2 },
                key: "val",
            })],
            want: Ok(value!({
                parent: {
                    child1: 1,
                    child2: { grandchild1: 1, grandchild2: 2 },
                },
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        map_and_array {
            args: func_args![value: value!({
                "parent.child1": [1, [2, 3]],
                "parent.child2.grandchild1": 1,
                "parent.child2.grandchild2": [1, [2, 3], 4],
                key: "val",
            })],
            want: Ok(value!({
                parent: {
                    child1: [1, [2, 3]],
                    child2: {grandchild1: 1, grandchild2: [1, [2, 3], 4]},
                },
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        map_and_array_with_separator {
            args: func_args![value: value!({
                "parent_child1": [1, [2, 3]],
                "parent_child2_grandchild1": 1,
                "parent_child2_grandchild2": [1, [2, 3], 4],
                key: "val",
            }), separator: "_"],
            want: Ok(value!({
                parent: {
                    child1: [1, [2, 3]],
                    child2: {grandchild1: 1, grandchild2: [1, [2, 3], 4]},
                },
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        // Objects inside arrays are not unflattened
        objects_inside_arrays {
            args: func_args![value: value!({
                "parent": [{"child1":1},{"child2.grandchild1": 1, "child2.grandchild2": 2 }],
                key: "val",
            })],
            want: Ok(value!({
                "parent": [{"child1":1},{"child2.grandchild1": 1, "child2.grandchild2": 2 }],
                key: "val",
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        triple_nested_map {
            args: func_args![value: value!({
                "parent1.child1.grandchild1": 1,
                "parent1.child2.grandchild2": 2,
                "parent1.child2.grandchild3": 3,
                parent2: 4,
            })],
            want: Ok(value!({
                parent1: {
                    child1: { grandchild1: 1 },
                    child2: { grandchild2: 2, grandchild3: 3 },
                },
                parent2: 4,
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        filter_out_top_level_value_when_multiple_values {
            args: func_args![value: value!({
                "a.b": 1,
                "a": 2,
            })],
            want: Ok(value!({
                a: { b: 1 },
            })),
            tdef: TypeDef::object(Collection::any()),
        }
    ];
}
