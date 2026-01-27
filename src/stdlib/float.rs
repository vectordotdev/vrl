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

    fn usage(&self) -> &'static str {
        "Returns `value` if it is a float, otherwise returns an error. This enables the type checker to guarantee that the returned value is a float and can be used in any function that expects a float."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a float."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a float.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare a float type",
                source: indoc! {r#"
                    . = { "value": 42.0 }
                    float(.value)
                "#},
                result: Ok("42.0"),
            },
            example! {
                title: "Declare a float type (literal)",
                source: "float(3.1415)",
                result: Ok("3.1415"),
            },
            example! {
                title: "Invalid float type",
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
