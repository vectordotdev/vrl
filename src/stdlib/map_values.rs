use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_RECURSIVE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The object or array to iterate.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "recursive",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether to recursively iterate the collection.",
            default: Some(&DEFAULT_RECURSIVE),
            enum_variants: None,
        },
    ]
});

fn map_values<T>(
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
        let value = match item {
            IterItem::KeyValue(_, value)
            | IterItem::IndexValue(_, value)
            | IterItem::Value(value) => value,
        };

        runner.map_value(ctx, value)?;
    }

    Ok(iter.into())
}

#[derive(Clone, Copy, Debug)]
pub struct MapValues;

impl Function for MapValues {
    fn identifier(&self) -> &'static str {
        "map_values"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Map the values within a collection.

            If `recursive` is enabled, the function iterates into nested
            collections, using the following rules:

            1. Iteration starts at the root.
            2. For every nested collection type:
               - First return the collection type itself.
               - Then recurse into the collection, and loop back to item (1)
                 in the list
               - Any mutation done on a collection *before* recursing into it,
                 are preserved.

            The function uses the function closure syntax to allow mutating
            the value for each item in the collection.

            The same scoping rules apply to closure blocks as they do for
            regular blocks, meaning, any variable defined in parent scopes
            are accessible, and mutations to those variables are preserved,
            but any new variables instantiated in the closure block are
            unavailable outside of the block.

            Check out the examples below to learn about the closure syntax.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY | kind::OBJECT
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Upcase values",
                source: indoc! {r#"
                    . = {
                        "foo": "foo",
                        "bar": "bar"
                    }
                    map_values(.) -> |value| { upcase(value) }
                "#},
                result: Ok(r#"{ "foo": "FOO", "bar": "BAR" }"#),
            },
            example! {
                title: "Recursively map object values",
                source: r#"map_values({ "a": 1, "b": [{ "c": 2 }, { "d": 3 }], "e": { "f": 4 } }, recursive: true) -> |value| { if is_integer(value) { int!(value) + 1 } else { value } }"#,
                result: Ok(r#"{ "a": 2, "b": [{ "c": 3 }, { "d": 4 }], "e": { "f": 5 } }"#),
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

        Ok(MapValuesFn {
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
                kind: Kind::object(Collection::any()).or_array(Collection::any()),
                variables: vec![Variable {
                    kind: VariableKind::TargetInnerValue,
                }],
                output: Output::Kind(Kind::any()),
                example: example! {
                    title: "map object values",
                    source: r#"map_values({ "one" : "one", "two": "two" }) -> |value| { upcase(value) }"#,
                    result: Ok(r#"{ "one": "ONE", "two": "TWO" }"#),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct MapValuesFn {
    value: Box<dyn Expression>,
    recursive: Option<Box<dyn Expression>>,
    closure: Closure,
}

impl FunctionExpression for MapValuesFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let recursive = self
            .recursive
            .map_resolve_with_default(ctx, || DEFAULT_RECURSIVE.clone())?
            .try_boolean()?;

        let value = self.value.resolve(ctx)?;
        let Closure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        map_values(value, recursive, ctx, &runner)
    }

    fn type_def(&self, ctx: &state::TypeState) -> TypeDef {
        let mut value = self.value.type_def(ctx);
        let closure = self.closure.block_type_def.kind().clone();

        recursive_type_def(&mut value, closure, true);
        value
    }
}

fn recursive_type_def(from: &mut Kind, to: Kind, root: bool) {
    if let Some(object) = from.as_object_mut() {
        for v in object.known_mut().values_mut() {
            recursive_type_def(v, to.clone(), false);
        }
    }

    if let Some(array) = from.as_array_mut() {
        for v in array.known_mut().values_mut() {
            recursive_type_def(v, to.clone(), false);
        }
    }

    if !root {
        *from = to;
    }
}
