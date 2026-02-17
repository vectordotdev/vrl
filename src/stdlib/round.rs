use crate::compiler::prelude::*;

use super::util::round_to_precision;
use std::sync::LazyLock;

static DEFAULT_PRECISION: LazyLock<Value> = LazyLock::new(|| Value::Integer(0));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::INTEGER | kind::FLOAT,
            required: true,
            description: "The number to round.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "precision",
            kind: kind::INTEGER,
            required: false,
            description: "The number of decimal places to round to.",
            default: Some(&DEFAULT_PRECISION),
            enum_variants: None,
        },
    ]
});

fn round(precision: Value, value: Value) -> Resolved {
    let precision = precision.try_integer()?;
    match value {
        Value::Float(f) => Ok(Value::from_f64_or_zero(round_to_precision(
            f.into_inner(),
            precision,
            f64::round,
        ))),
        value @ Value::Integer(_) => Ok(value),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::float() | Kind::integer(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Round;

impl Function for Round {
    fn identifier(&self) -> &'static str {
        "round"
    }

    fn usage(&self) -> &'static str {
        "Rounds the `value` to the specified `precision`."
    }

    fn category(&self) -> &'static str {
        Category::Number.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER | kind::FLOAT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &["If `precision` is `0`, then an integer is returned, otherwise a float is returned."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Round a number (without precision)",
                source: "round(4.345)",
                result: Ok("4.0"),
            },
            example! {
                title: "Round a number (with precision)",
                source: "round(4.345, precision: 2)",
                result: Ok("4.35"),
            },
            example! {
                title: "Round up",
                source: "round(5.5)",
                result: Ok("6.0"),
            },
            example! {
                title: "Round down",
                source: "round(5.45)",
                result: Ok("5.0"),
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
        let precision = arguments.optional("precision");

        Ok(RoundFn { value, precision }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct RoundFn {
    value: Box<dyn Expression>,
    precision: Option<Box<dyn Expression>>,
}

impl FunctionExpression for RoundFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let precision = self
            .precision
            .map_resolve_with_default(ctx, || DEFAULT_PRECISION.clone())?;
        let value = self.value.resolve(ctx)?;

        round(precision, value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        round => Round;

        down {
             args: func_args![value: 1234.2],
             want: Ok(1234.0),
             tdef: TypeDef::integer().infallible(),
         }

        up {
             args: func_args![value: 1234.8],
             want: Ok(1235.0),
             tdef: TypeDef::integer().infallible(),
         }

        integer {
             args: func_args![value: 1234],
             want: Ok(1234),
             tdef: TypeDef::integer().infallible(),
         }

        precision {
             args: func_args![value: 1234.39429,
                              precision: 1
             ],
             want: Ok(1234.4),
             tdef: TypeDef::integer().infallible(),
         }

        bigger_precision  {
            args: func_args![value: 1234.56789,
                             precision: 4
            ],
            want: Ok(1234.5679),
            tdef: TypeDef::integer().infallible(),
        }

        huge {
             args: func_args![value: 9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_654_321,
                              precision: 5
             ],
             want: Ok(9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_65),
             tdef: TypeDef::integer().infallible(),
         }
    ];
}
