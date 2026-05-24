use super::util::round_to_precision;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_PRECISION: LazyLock<Value> = LazyLock::new(|| Value::Integer(0));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required(
            "value",
            kind::INTEGER | kind::FLOAT | kind::DECIMAL,
            "The number to round.",
        ),
        Parameter::optional(
            "precision",
            kind::INTEGER,
            "The number of decimal places to round to.",
        )
        .default(&DEFAULT_PRECISION),
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
        Value::Decimal(d) => {
            let dp = u32::try_from(precision.max(0)).unwrap_or(u32::MAX);
            Ok(Value::Decimal(d.round_dp_with_strategy(
                dp,
                rust_decimal::RoundingStrategy::MidpointAwayFromZero,
            )))
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::float() | Kind::integer() | Kind::decimal(),
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
        kind::INTEGER | kind::FLOAT | kind::DECIMAL
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
            example! {
                title: "Round a decimal",
                source: "round(d'4.5')",
                result: Ok("d'5'"),
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

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        // Decimals preserve their type when rounded
        match Kind::from(self.value.type_def(state)) {
            v if v.is_float() => TypeDef::integer().infallible(),
            v if v.is_integer() => TypeDef::integer().infallible(),
            v if v.is_decimal() => TypeDef::decimal().infallible(),
            _ => TypeDef::integer().or_decimal().infallible(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

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

        decimal_down {
            args: func_args![value: Value::Decimal(dec!(1234.2))],
            want: Ok(Value::Decimal(dec!(1234))),
            tdef: TypeDef::decimal().infallible(),
        }

        decimal_up {
            args: func_args![value: Value::Decimal(dec!(1234.6))],
            want: Ok(Value::Decimal(dec!(1235))),
            tdef: TypeDef::decimal().infallible(),
        }

        decimal_precision {
            args: func_args![value: Value::Decimal(dec!(1234.39429)), precision: 1],
            want: Ok(Value::Decimal(dec!(1234.4))),
            tdef: TypeDef::decimal().infallible(),
        }

        decimal_bigger_precision {
            args: func_args![value: Value::Decimal(dec!(1234.56789)), precision: 4],
            want: Ok(Value::Decimal(dec!(1234.5679))),
            tdef: TypeDef::decimal().infallible(),
        }
    ];
}
