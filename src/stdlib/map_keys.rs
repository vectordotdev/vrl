use crate::compiler::prelude::*;

fn map_keys<T>(
    value: Value,
    recursive: bool,
    ctx: &mut Context,
    runner: &closure::Runner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Resolved,
{
    let mut iter = value.into_iter(recursive);

    for item in iter.by_ref() {
        if let IterItem::KeyValue(key, _) = item {
            runner.map_key(ctx, key)?;
        }
    }

    Ok(iter.into())
}

#[derive(Clone, Copy, Debug)]
pub struct MapKeys;

impl Function for MapKeys {
    fn identifier(&self) -> &'static str {
        "map_keys"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Map the keys within an object.

            If `recursive` is enabled, the function iterates into nested
            objects, using the following rules:

            1. Iteration starts at the root.
            2. For every nested object type:
               - First return the key of the object type itself.
               - Then recurse into the object, and loop back to item (1)
                 in this list.
               - Any mutation done on a nested object *before* recursing into
                 it, are preserved.
            3. For every nested array type:
               - First return the key of the array type itself.
               - Then find all objects within the array, and apply item (2)
                 to each individual object.

            The above rules mean that `map_keys` with
            `recursive` enabled finds *all* keys in the target,
            regardless of whether nested objects are nested inside arrays.

            The function uses the function closure syntax to allow reading
            the key for each item in the object.

            The same scoping rules apply to closure blocks as they do for
            regular blocks. This means that any variable defined in parent scopes
            is accessible, and mutations to those variables are preserved,
            but any new variables instantiated in the closure block are
            unavailable outside of the block.

            See the examples below to learn about the closure syntax.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT,
                required: true,
                description: "The object to iterate.",
            },
            Parameter {
                keyword: "recursive",
                kind: kind::BOOLEAN,
                required: false,
                description: "Whether to recursively iterate the collection.",
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Upcase keys",
                source: indoc! {r#"
                    . = {
                        "foo": "foo",
                        "bar": "bar",
                        "baz": {"nested key": "val"}
                    }
                    map_keys(.) -> |key| { upcase(key) }
                "#},
                result: Ok(r#"{ "FOO": "foo", "BAR": "bar", "BAZ": {"nested key": "val"} }"#),
            },
            example! {
                title: "De-dot keys",
                source: indoc! {r#"
                    . = {
                        "labels": {
                            "app.kubernetes.io/name": "mysql"
                        }
                    }
                    map_keys(., recursive: true) -> |key| { replace(key, ".", "_") }
                "#},
                result: Ok(r#"{ "labels": { "app_kubernetes_io/name": "mysql" } }"#),
            },
            example! {
                title: "Recursively map object keys",
                source: r#"map_keys({ "a": 1, "b": [{ "c": 2 }, { "d": 3 }], "e": { "f": 4 } }, recursive: true) -> |key| { upcase(key) }"#,
                result: Ok(r#"{ "A": 1, "B": [{ "C": 2 }, { "D": 3 }], "E": { "F": 4 } }"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let recursive = arguments.optional("recursive");
        let closure = arguments.required_closure()?;

        Ok(MapKeysFn {
            value,
            recursive,
            closure,
        }
        .as_expr())
    }

    fn closure(&self) -> Option<closure::Definition> {
        use closure::{Definition, Input, Output, Variable, VariableKind};

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::object(Collection::any()),
                variables: vec![Variable {
                    kind: VariableKind::Exact(Kind::bytes()),
                }],
                output: Output::Kind(Kind::bytes()),
                example: example! {
                    title: "map object keys",
                    source: r#"map_keys({ "one" : 1, "two": 2 }) -> |key| { upcase(key) }"#,
                    result: Ok(r#"{ "ONE": 1, "TWO": 2 }"#),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct MapKeysFn {
    value: Box<dyn Expression>,
    recursive: Option<Box<dyn Expression>>,
    closure: Closure,
}

impl FunctionExpression for MapKeysFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let recursive = match &self.recursive {
            None => false,
            Some(expr) => expr.resolve(ctx)?.try_boolean()?,
        };

        let value = self.value.resolve(ctx)?;
        let Closure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        map_keys(value, recursive, ctx, &runner)
    }

    fn type_def(&self, ctx: &state::TypeState) -> TypeDef {
        self.value.type_def(ctx)
    }
}
