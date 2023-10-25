use crate::compiler::prelude::*;

fn float(value: Value) -> Resolved {
    match value {
        v @ Value::Float(_) => Ok(v),
        v => Err(format!("expected float, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Float;

impl Function for Float {
    fn identifier(&self) -> &'static str {
        "float"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid",
                source: r#"float(3.1415)"#,
                result: Ok("3.1415"),
            },
            Example {
                title: "invalid",
                source: "float!(true)",
                result: Err(
                    r#"function call error for "float" at (0:12): expected float, got boolean"#,
                ),
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

        Ok(FloatFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct FloatFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for FloatFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        float(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let non_float = !self.value.type_def(state).is_float();

        TypeDef::float().maybe_fallible(non_float)
    }
}
