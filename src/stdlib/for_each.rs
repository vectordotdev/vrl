use crate::compiler::prelude::*;

fn for_each<T>(value: Value, ctx: &mut Context, runner: &closure::Runner<T>) -> Resolved
where
    T: Fn(&mut Context) -> Resolved,
{
    for item in value.into_iter(false) {
        match item {
            IterItem::KeyValue(key, value) => {
                runner.run_key_value(ctx, key, value)?;
            }
            IterItem::IndexValue(index, value) => {
                runner.run_index_value(ctx, index, value)?;
            }
            IterItem::Value(_) => {}
        }
    }

    Ok(Value::Null)
}

#[derive(Clone, Copy, Debug)]
pub struct ForEach;

impl Function for ForEach {
    fn identifier(&self) -> &'static str {
        "for_each"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Iterate over a collection.

            This function currently *does not* support recursive iteration.

            The function uses the \"function closure syntax\" to allow reading
            the key/value or index/value combination for each item in the
            collection.

            The same scoping rules apply to closure blocks as they do for
            regular blocks. This means that any variable defined in parent scopes
            is accessible, and mutations to those variables are preserved,
            but any new variables instantiated in the closure block are
            unavailable outside of the block.

            See the examples below to learn about the closure syntax.
        "}
    }
    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The array or object to iterate.",
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Tally elements",
                source: indoc! {r#"
                    .tags = ["foo", "bar", "foo", "baz"]
                    tally = {}
                    for_each(array(.tags)) -> |_index, value| {
                        count = int(get!(tally, [value])) ?? 0
                        tally = set!(tally, [value], count + 1)
                    }
                    tally
                "#},
                result: Ok(r#"{"bar": 1, "baz": 1, "foo": 2}"#),
            },
            example! {
                title: "Iterate over an object",
                source: r#"count = 0; for_each({ "a": 1, "b": 2 }) -> |_key, value| { count = count + value }; count"#,
                result: Ok("3"),
            },
            example! {
                title: "Iterate over an array",
                source: "count = 0; for_each([1,2,3]) -> |index, value| { count = count + index + value }; count",
                result: Ok("9"),
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

        Ok(ForEachFn { value, closure }.as_expr())
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
                output: Output::Kind(Kind::any()),
                example: example! {
                    title: "iterate array",
                    source: "for_each([1, 2]) -> |index, value| { .foo = to_int!(.foo) + index + value }",
                    result: Ok("null"),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct ForEachFn {
    value: Box<dyn Expression>,
    closure: Closure,
}

impl FunctionExpression for ForEachFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let value = self.value.resolve(ctx)?;
        let Closure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::Runner::new(variables, |ctx| block.resolve(ctx));

        for_each(value, ctx, &runner)
    }

    fn type_def(&self, _ctx: &state::TypeState) -> TypeDef {
        TypeDef::null()
    }
}
