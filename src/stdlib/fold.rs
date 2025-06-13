use crate::compiler::prelude::*;

fn fold<T>(
    value: Value,
    initial_value: Value,
    ctx: &mut Context,
    runner: &closure::FluentRunner<T>,
) -> Resolved
where
    T: Fn(&mut Context) -> Resolved,
{
    let mut swap_space: [Option<Value>; 3] = [None, None, None];
    match value {
        Value::Object(object) => {
            object
                .into_iter()
                .try_fold(initial_value, |accum, (key, value)| {
                    runner
                        .with_swap_space(&mut swap_space)
                        .parameter(ctx, 0, accum)
                        .parameter(ctx, 1, key.into())
                        .parameter(ctx, 2, value)
                        .run(ctx)
                })
        }

        Value::Array(array) => {
            array
                .into_iter()
                .enumerate()
                .try_fold(initial_value, |accum, (index, value)| {
                    runner
                        .with_swap_space(&mut swap_space)
                        .parameter(ctx, 0, accum)
                        .parameter(ctx, 1, index.into())
                        .parameter(ctx, 2, value)
                        .run(ctx)
                })
        }

        _ => Err("function requires collection types as input".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Fold;

impl Function for Fold {
    fn identifier(&self) -> &'static str {
        "fold"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT | kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "initial_value",
                kind: kind::ANY,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "fold array",
                source: r"fold([false, false, true], false) -> |accum, _index, value| { value && accum }",
                result: Ok("false"),
            },
            Example {
                title: "fold object",
                source: r#"fold({"first_key": 0, "second_key": 1}, 0) -> |accum, key, _value| { strlen(key) + accum }"#,
                result: Ok("19"),
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
        let initial_value = arguments.required("initial_value");
        let closure = arguments.required_closure()?;

        Ok(FoldFn {
            value,
            initial_value,
            closure,
        }
        .as_expr())
    }

    fn closure(&self) -> Option<closure::Definition> {
        use closure::{Definition, InitialKind, Input, Output, Variable, VariableKind};

        Some(Definition {
            inputs: vec![Input {
                parameter_keyword: "value",
                kind: Kind::object(Collection::any()).or_array(Collection::any()),
                variables: vec![
                    Variable {
                        kind: VariableKind::Closure(InitialKind::Parameter("initial_value")),
                    },
                    Variable {
                        kind: VariableKind::TargetInnerKey,
                    },
                    Variable {
                        kind: VariableKind::TargetInnerValue,
                    },
                ],
                output: Output::Kind(Kind::any()),
                example: Example {
                    title: "fold array",
                    source: r"fold([15, 40, 35], 20) -> |accum, _index, value| { if value > accum { value } else { accum } }",
                    result: Ok(r"40"),
                },
            }],
            is_iterator: true,
        })
    }
}

#[derive(Debug, Clone)]
struct FoldFn {
    value: Box<dyn Expression>,
    initial_value: Box<dyn Expression>,
    closure: Closure,
}

impl FunctionExpression for FoldFn {
    fn resolve(&self, ctx: &mut Context) -> ExpressionResult<Value> {
        let value = self.value.resolve(ctx)?;
        let initial_value = self.initial_value.resolve(ctx)?;

        let Closure {
            variables,
            block,
            block_type_def: _,
        } = &self.closure;
        let runner = closure::FluentRunner::new(variables, |ctx| block.resolve(ctx));

        fold(value, initial_value, ctx, &runner)
    }

    fn type_def(&self, _ctx: &state::TypeState) -> TypeDef {
        self.closure.block_type_def.kind().clone().into()
    }
}
