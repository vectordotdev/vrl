use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsTimestamp;

impl Function for IsTimestamp {
    fn identifier(&self) -> &'static str {
        "is_timestamp"
    }

    fn usage(&self) -> &'static str {
        "Check if `value`'s type is a timestamp."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is a timestamp.",
            "Returns `false` if `value` is anything else.",
        ]
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
                title: "Valid timestamp",
                source: "is_timestamp(t'2021-03-26T16:00:00Z')",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_timestamp("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Boolean value",
                source: "is_timestamp(true)",
                result: Ok("false"),
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

        Ok(IsTimestampFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsTimestampFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_timestamp()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    test_function![
        is_timestamp => IsTimestamp;

        timestamp {
            args: func_args![value: value!(DateTime::parse_from_rfc2822("Wed, 17 Mar 2021 12:00:00 +0000")
                .unwrap()
                .with_timezone(&Utc))],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        integer {
            args: func_args![value: value!(1789)],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
