use crate::compiler::prelude::*;
use std::collections::BTreeMap;

fn filter<T>(value: Value, ctx: &mut Context, runner: &closure::Runner<T>) -> Resolved
where
    T: Fn(&mut Context) -> Resolved,
{
    match value {
        Value::Object(object) => object
            .into_iter()
            .filter_map(
                |(key, value)| match runner.run_key_value(ctx, &key, &value) {
                    Ok(v) => v
                        .as_boolean()
                        .expect("compiler guarantees boolean return type")
                        .then_some(Ok((key, value))),
                    Err(err) => Some(Err(err)),
                },
            )
            .collect::<ExpressionResult<BTreeMap<_, _>>>()
            .map(Into::into),

        Value::Array(array) => array
            .into_iter()
            .enumerate()
            .filter_map(
                |(index, value)| match runner.run_index_value(ctx, index, &value) {
                    Ok(v) => v
                        .as_boolean()
                        .expect("compiler guarantees boolean return type")
                        .then_some(Ok(value)),
                    Err(err) => Some(Err(err)),
                },
            )
            .collect::<ExpressionResult<Vec<_>>>()
            .map(Into::into),

        _ => Err("function requires collection types as input".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Filter;

impl Function for Filter {
    fn identifier(&self) -> &'static str {
        "filter"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Filter elements from a collection.

            This function currently *does not* support recursive iteration.

            The function uses the function closure syntax to allow reading
            the key-value or index-value combination for each item in the
            collection.

            The same scoping rules apply to closure blocks as they do for
            regular blocks. This means that any variable defined in parent scopes
            is accessible, and mutations to those variables are preserved,
            but any new variables instantiated in the closure block are
            unavailable outside of the block.

            See the examples below to learn about the closure syntax.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY | kind::OBJECT
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The array or object to filter.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Filter elements",
                source: indoc! {r#"
                    . = { "tags": ["foo", "bar", "foo", "baz"] }
                    filter(array(.tags)) -> |_index, value| {
                        value != "foo"
                    }
                "#},
                result: Ok(r#"["bar", "baz"]"#),
            },
            example! {
                title: "Filter object",
                source: r#"filter({ "a": 1, "b": 2 }) -> |key, _value| { key == "a" }"#,
                result: Ok(r#"{ "a": 1 }"#),
            },
            example! {
                title: "Filter array",
                source: "filter([1, 2]) -> |_index, value| { value < 2 }",
                result: Ok("[1]"),
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
        let closure = arguments.required_closure()?;

        Ok(FilterFn { value, closure }.as_expr())
    }

    fn closure(&self) -> Option<closure::Definition> {
        use closure::{Definition, Input, Output, Variable, VariableKind};

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::object(Collection::any()).or_array(Collection::any()),
                variables: vec![
                    Variable {
                        kind: VariableKind::TargetInnerKey,
                    },
                    Variable {
                        kind: VariableKind::TargetInnerValue,
                    },
                ],
                output: Output::Kind(Kind::boolean()),
                example: example! {
                    title: "filter array",
                    source: "filter([1, 2]) -> |index, _value| { index == 0 }",
                    result: Ok("[1]"),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct FilterFn {
    value: Box<dyn Expression>,
    closure: Closure,
}

impl FunctionExpression for FilterFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let value = self.value.resolve(ctx)?;
        let Closure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        filter(value, ctx, &runner)
    }

    fn type_def(&self, ctx: &state::TypeState) -> TypeDef {
        let mut type_def = self.value.type_def(ctx);

        // Erase any type information from the array or object, as we can't know
        // which elements are removed at runtime.
        if type_def.contains_array() {
            type_def.kind_mut().add_array(Collection::any());
        }

        if type_def.contains_object() {
            type_def.kind_mut().add_object(Collection::any());
        }

        type_def
    }
}
