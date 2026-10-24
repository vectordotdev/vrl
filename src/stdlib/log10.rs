use crate::compiler::prelude::*;

#[allow(clippy::cast_precision_loss)] // integers beyond 2^53 lose precision, acceptable for a logarithm
fn log10(value: Value) -> Resolved {
    let f = match value {
        Value::Float(f) => f.into_inner(),
        Value::Integer(i) => i as f64,
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::float() | Kind::integer(),
            }
            .into());
        }
    };
    if f <= 0.0 {
        return Err(format!("cannot compute log10 of non-positive number {f}").into());
    }
    Ok(Value::from_f64_or_zero(f.log10()))
}

#[derive(Clone, Copy, Debug)]
pub struct Log10;

impl Function for Log10 {
    fn identifier(&self) -> &'static str {
        "log10"
    }

    fn usage(&self) -> &'static str {
        "Computes the base 10 logarithm of `value`."
    }

    fn category(&self) -> &'static str {
        Category::Number.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is less than or equal to 0."]
    }

    fn return_kind(&self) -> u16 {
        kind::FLOAT
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::FLOAT | kind::INTEGER,
            "The number to calculate the base 10 logarithm of.",
        )];
        PARAMETERS
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(Log10Fn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Compute the base 10 logarithm of an integer",
                source: "log10!(100)",
                result: Ok("2.0"),
            },
            example! {
                title: "Convert milliwatts to dBm",
                source: "10 * log10!(0.5)",
                result: Ok("-3.010299956639812"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct Log10Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for Log10Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        log10(self.value.resolve(ctx)?)
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
        log10 => Log10;

        integer {
            args: func_args![value: value!(1000)],
            want: Ok(value!(3.0)),
            tdef: TypeDef::float().fallible(),
        }

        float {
            args: func_args![value: value!(0.01)],
            want: Ok(value!(-2.0)),
            tdef: TypeDef::float().fallible(),
        }

        zero {
            args: func_args![value: value!(0)],
            want: Err("cannot compute log10 of non-positive number 0"),
            tdef: TypeDef::float().fallible(),
        }

        negative {
            args: func_args![value: value!(-1.5)],
            want: Err("cannot compute log10 of non-positive number -1.5"),
            tdef: TypeDef::float().fallible(),
        }
    ];
}
