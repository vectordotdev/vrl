use std::collections::HashSet;
use std::collections::btree_map;

use crate::compiler::expression::Expr;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_SEPARATOR: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from(".")));
static DEFAULT_EXCEPT: LazyLock<Value> = LazyLock::new(|| Value::Array(vec![]));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required(
            "value",
            kind::OBJECT | kind::ARRAY,
            "The array or object to flatten.",
        ),
        Parameter::optional(
            "separator",
            kind::BYTES,
            "The separator to join nested keys",
        )
        .default(&DEFAULT_SEPARATOR),
        Parameter::optional(
            "except",
            kind::ARRAY,
            "An array of key names to exclude from flattening at any depth.",
        )
        .default(&DEFAULT_EXCEPT),
    ]
});

fn flatten(value: Value, separator: &Value, except: &HashSet<KeyString>) -> Resolved {
    let separator = separator.try_bytes_utf8_lossy()?;

    match value {
        Value::Array(arr) => Ok(Value::Array(
            ArrayFlatten::new(arr.iter()).cloned().collect(),
        )),
        Value::Object(map) => Ok(Value::Object(
            MapFlatten::new(map.iter(), &separator, except)
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

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY | kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &["The return type matches the `value` type."]
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
                source: indoc! {r#"
                    flatten({
                        "parent1": {
                            "child1": 1,
                            "child2": 2
                        },
                        "parent2": {
                            "child3": 3
                        }
                    })
                "#},
                result: Ok(r#"{ "parent1.child1": 1, "parent1.child2": 2, "parent2.child3": 3 }"#),
            },
            example! {
                title: "Flatten object with custom separator",
                source: r#"flatten({ "foo": { "bar": true }}, "_")"#,
                result: Ok(r#"{ "foo_bar": true }"#),
            },
            example! {
                title: "Flatten object with except",
                source: r#"flatten({ "parent": { "child": 1 }, "keep": { "nested": 2 } }, except: ["keep"])"#,
                result: Ok(r#"{ "keep": { "nested": 2 }, "parent.child": 1 }"#),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let separator = arguments.optional("separator");
        let value = arguments.required("value");

        let except = arguments
            .optional_array("except")?
            .map(|arr| {
                arr.into_iter()
                    .map(|expr| into_key(state, expr))
                    .collect::<Result<HashSet<KeyString>, _>>()
            })
            .transpose()?;

        Ok(FlattenFn {
            value,
            separator,
            except,
        }
        .as_expr())
    }
}

fn into_key(state: &state::TypeState, expr: Expr) -> Result<KeyString, function::Error> {
    let v = expr
        .resolve_constant(state)
        .ok_or(function::Error::ExpectedStaticExpression {
            keyword: "except",
            expr,
        })?;

    match v.try_bytes_utf8_lossy() {
        Ok(s) => Ok(KeyString::from(s.into_owned())),
        Err(_) => Err(function::Error::InvalidArgument {
            keyword: "except",
            value: v,
            error: "expected a string value",
        }),
    }
}

#[derive(Debug, Clone)]
struct FlattenFn {
    value: Box<dyn Expression>,
    separator: Option<Box<dyn Expression>>,
    except: Option<HashSet<KeyString>>,
}

impl FunctionExpression for FlattenFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let separator = self
            .separator
            .map_resolve_with_default(ctx, || DEFAULT_SEPARATOR.clone())?;
        let empty = HashSet::new();
        let except = self.except.as_ref().unwrap_or(&empty);

        flatten(value, &separator, except)
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
    except: &'a HashSet<KeyString>,
}

impl<'a> MapFlatten<'a> {
    fn new(
        values: btree_map::Iter<'a, KeyString, Value>,
        separator: &'a str,
        except: &'a HashSet<KeyString>,
    ) -> Self {
        Self {
            values,
            separator,
            inner: None,
            parent: None,
            except,
        }
    }

    fn new_from_parent(
        parent: KeyString,
        values: btree_map::Iter<'a, KeyString, Value>,
        separator: &'a str,
        except: &'a HashSet<KeyString>,
    ) -> Self {
        Self {
            values,
            separator,
            inner: None,
            parent: Some(parent),
            except,
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
            Some((key, Value::Object(value))) if !self.except.contains(key) => {
                self.inner = Some(Box::new(MapFlatten::new_from_parent(
                    self.new_key(key),
                    value.iter(),
                    self.separator,
                    self.except,
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

        nested_map_with_except {
            args: func_args![
                value: value!({parent: {child1: 1, child2: 2}, keep: {nested: 3}, key: "val"}),
                except: value!(["keep"])
            ],
            want: Ok(value!({"parent.child1": 1, "parent.child2": 2, keep: {nested: 3}, key: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map_with_except_and_separator {
            args: func_args![
                value: value!({parent: {child1: 1}, keep: {nested: 2}}),
                separator: "_",
                except: value!(["keep"])
            ],
            want: Ok(value!({"parent_child1": 1, keep: {nested: 2}})),
            tdef: TypeDef::object(Collection::any()),
        }

        nested_map_with_multiple_except {
            args: func_args![
                value: value!({a: {b: 1}, c: {d: 2}, e: {f: 3}}),
                except: value!(["a", "e"])
            ],
            want: Ok(value!({a: {b: 1}, "c.d": 2, e: {f: 3}})),
            tdef: TypeDef::object(Collection::any()),
        }

        except_nonexistent_key {
            args: func_args![
                value: value!({parent: {child: 1}}),
                except: value!(["nonexistent"])
            ],
            want: Ok(value!({"parent.child": 1})),
            tdef: TypeDef::object(Collection::any()),
        }

        except_empty_array {
            args: func_args![
                value: value!({parent: {child: 1}}),
                except: value!([])
            ],
            want: Ok(value!({"parent.child": 1})),
            tdef: TypeDef::object(Collection::any()),
        }

        except_non_object_key {
            args: func_args![
                value: value!({parent: {child: 1}, leaf: "val"}),
                except: value!(["leaf"])
            ],
            want: Ok(value!({"parent.child": 1, leaf: "val"})),
            tdef: TypeDef::object(Collection::any()),
        }

        except_any_depth {
            args: func_args![
                value: value!({
                    keep: {nested: 1},
                    parent: {keep: {deep: 2}},
                }),
                except: value!(["keep"])
            ],
            want: Ok(value!({
                keep: {nested: 1},
                "parent.keep": {deep: 2},
            })),
            tdef: TypeDef::object(Collection::any()),
        }

        array_with_except {
            args: func_args![value: value!([1, [2, 3]]), except: value!(["key"])],
            want: Ok(value!([1, 2, 3])),
            tdef: TypeDef::array(Collection::any()),
        }
    ];
}
