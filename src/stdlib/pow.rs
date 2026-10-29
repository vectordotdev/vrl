use crate::compiler::prelude::*;

#[allow(clippy::cast_precision_loss)] // integers beyond 2^53 lose precision, acceptable for a power
fn to_f64(value: Value) -> Result<f64, ExpressionError> {
    match value {
        Value::Float(f) => Ok(f.into_inner()),
        Value::Integer(i) => Ok(i as f64),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::float() | Kind::integer(),
        }
        .into()),
    }
}

fn pow(base: Value, exponent: Value) -> Resolved {
    // An odd integer exponent above 2^53 can round to an even f64, so take the sign from its parity.
    let odd_exponent = matches!(exponent, Value::Integer(e) if e % 2 != 0);
    let base = to_f64(base)?;
    let exponent = to_f64(exponent)?;
    let result = if odd_exponent && base < 0.0 {
        -(-base).powf(exponent)
    } else {
        base.powf(exponent)
    };
    if !result.is_finite() {
        return Err(
            format!("{base} raised to the power of {exponent} is not a finite number").into(),
        );
    }
    Ok(Value::from_f64_or_zero(result))
}

#[derive(Clone, Copy, Debug)]
pub struct Pow;

impl Function for Pow {
    fn identifier(&self) -> &'static str {
        "pow"
    }

    fn usage(&self) -> &'static str {
        "Raises `base` to the power of `exponent`."
    }

    fn category(&self) -> &'static str {
        Category::Number.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "The result is not a finite number, for example a negative `base` with a fractional `exponent`, `0` with a negative `exponent`, or an overflow.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::FLOAT
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required("base", kind::FLOAT | kind::INTEGER, "The base."),
            Parameter::required("exponent", kind::FLOAT | kind::INTEGER, "The exponent."),
        ];
        PARAMETERS
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let base = arguments.required("base");
        let exponent = arguments.required("exponent");

        Ok(PowFn { base, exponent }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Raise an integer to a power",
                source: "pow!(2, 10)",
                result: Ok("1024.0"),
            },
            example! {
                title: "Convert dBm to milliwatts",
                source: "pow!(10, -3.0 / 10)",
                result: Ok("0.5011872336272722"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct PowFn {
    base: Box<dyn Expression>,
    exponent: Box<dyn Expression>,
}

impl FunctionExpression for PowFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let base = self.base.resolve(ctx)?;
        let exponent = self.exponent.resolve(ctx)?;
        pow(base, exponent)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::float().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        pow => Pow;

        integers {
            args: func_args![base: value!(2), exponent: value!(3)],
            want: Ok(value!(8.0)),
            tdef: TypeDef::float().fallible(),
        }

        negative_exponent {
            args: func_args![base: value!(10), exponent: value!(-2)],
            want: Ok(value!(0.01)),
            tdef: TypeDef::float().fallible(),
        }

        square_root {
            args: func_args![base: value!(9.0), exponent: value!(0.5)],
            want: Ok(value!(3.0)),
            tdef: TypeDef::float().fallible(),
        }

        odd_exponent_beyond_f64_precision {
            args: func_args![base: value!(-1), exponent: value!(9_007_199_254_740_993_i64)],
            want: Ok(value!(-1.0)),
            tdef: TypeDef::float().fallible(),
        }

        nan {
            args: func_args![base: value!(-8.0), exponent: value!(0.5)],
            want: Err("-8 raised to the power of 0.5 is not a finite number"),
            tdef: TypeDef::float().fallible(),
        }

        zero_negative_exponent {
            args: func_args![base: value!(0), exponent: value!(-1)],
            want: Err("0 raised to the power of -1 is not a finite number"),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
