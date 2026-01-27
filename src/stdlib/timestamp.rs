use crate::compiler::prelude::*;

fn timestamp(value: Value) -> Resolved {
    match value {
        v @ Value::Timestamp(_) => Ok(v),
        v => Err(format!("expected timestamp, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Timestamp;

impl Function for Timestamp {
    fn identifier(&self) -> &'static str {
        "timestamp"
    }

    fn usage(&self) -> &'static str {
        "Returns `value` if it is a timestamp, otherwise returns an error. This enables the type checker to guarantee that the returned value is a timestamp and can be used in any function that expects a timestamp."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a timestamp."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a timestamp.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare a timestamp type",
                source: "timestamp(t'2020-10-10T16:00:00Z')",
                result: Ok("t'2020-10-10T16:00:00Z'"),
            },
            example! {
                title: "Invalid type",
                source: "timestamp!(true)",
                result: Err(
                    r#"function call error for "timestamp" at (0:16): expected timestamp, got boolean"#,
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

        Ok(TimestampFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct TimestampFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for TimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        timestamp(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let non_timestamp = !self.value.type_def(state).is_timestamp();

        TypeDef::timestamp().maybe_fallible(non_timestamp)
    }
}
