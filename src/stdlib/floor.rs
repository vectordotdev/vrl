use super::util::round_to_precision;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_PRECISION: LazyLock<Value> = LazyLock::new(|| Value::Integer(0));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required(
            "value",
            kind::FLOAT | kind::INTEGER | kind::DECIMAL,
            "The number to round down.",
        ),
        Parameter::optional(
            "precision",
            kind::INTEGER,
            "The number of decimal places to round to.",
        )
        .default(&DEFAULT_PRECISION),
    ]
});

fn floor(precision: Value, value: Value) -> Resolved {
    let precision = precision.try_integer()?;

    match value {
        Value::Float(f) => Ok(Value::from_f64_or_zero(round_to_precision(
            *f,
            precision,
            f64::floor,
        ))),
        value @ Value::Integer(_) => Ok(value),
        Value::Decimal(d) => {
            let dp = u32::try_from(precision.max(0)).unwrap_or(u32::MAX);
            Ok(Value::Decimal(d.round_dp_with_strategy(
                dp,
                rust_decimal::RoundingStrategy::ToNegativeInfinity,
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
pub struct Floor;

impl Function for Floor {
    fn identifier(&self) -> &'static str {
        "floor"
    }

    fn usage(&self) -> &'static str {
        "Rounds the `value` down to the specified `precision`."
    }

    fn category(&self) -> &'static str {
        Category::Number.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER | kind::FLOAT | kind::DECIMAL
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns an integer if `precision` is `0` (this is the default). Returns a float otherwise.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let precision = arguments.optional("precision");

        Ok(FloorFn { value, precision }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Round a number down (without precision)",
                source: "floor(9.8)",
                result: Ok("9.0"),
            },
            example! {
                title: "Round a number down (with precision)",
                source: "floor(4.345, precision: 2)",
                result: Ok("4.34"),
            },
            example! {
                title: "Round a decimal down",
                source: "floor(d'4.345')",
                result: Ok("d'4'"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct FloorFn {
    value: Box<dyn Expression>,
    precision: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FloorFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let precision = self
            .precision
            .map_resolve_with_default(ctx, || DEFAULT_PRECISION.clone())?;
        let value = self.value.resolve(ctx)?;

        floor(precision, value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        match Kind::from(self.value.type_def(state)) {
            v if v.is_float() => v.into(),
            v if v.is_integer() => v.into(),
            v if v.is_decimal() => v.into(),
            _ => Kind::integer().or_float().or_decimal().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use rust_decimal::dec;

    test_function![
        floor => Floor;

        lower {
            args: func_args![value: 1234.2],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        higher {
            args: func_args![value: 1234.8],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        exact {
            args: func_args![value: 1234],
            want: Ok(value!(1234)),
            tdef: TypeDef::integer(),
        }

        precision {
            args: func_args![value: 1234.39429,
                             precision: 1],
            want: Ok(value!(1234.3)),
            tdef: TypeDef::float(),
        }

        bigger_precision {
            args: func_args![value: 1234.56789,
                             precision: 4],
            want: Ok(value!(1234.5678)),
            tdef: TypeDef::float(),
        }

        huge_number {
            args: func_args![value: 9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_654_321,
                             precision: 5],
            want: Ok(value!(9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_65)),
            tdef: TypeDef::float(),
        }

        decimal_lower {
            args: func_args![value: Value::Decimal(dec!(1234.2))],
            want: Ok(Value::Decimal(dec!(1234))),
            tdef: TypeDef::decimal(),
        }

        decimal_higher {
            args: func_args![value: Value::Decimal(dec!(1234.8))],
            want: Ok(Value::Decimal(dec!(1234))),
            tdef: TypeDef::decimal(),
        }

        decimal_precision {
            args: func_args![value: Value::Decimal(dec!(1234.39429)), precision: 1],
            want: Ok(Value::Decimal(dec!(1234.3))),
            tdef: TypeDef::decimal(),
        }

        decimal_bigger_precision {
            args: func_args![value: Value::Decimal(dec!(1234.56789)), precision: 4],
            want: Ok(Value::Decimal(dec!(1234.5678))),
            tdef: TypeDef::decimal(),
        }
    ];
}
