use crate::compiler::prelude::*;
use rust_decimal::Decimal;

fn to_decimal(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null};
    match value {
        Value::Decimal(_) => Ok(value),
        Integer(v) => Ok(Value::Decimal(Decimal::from(v))),
        Float(v) => v
            .into_inner()
            .to_string()
            .parse::<Decimal>()
            .map(Value::Decimal)
            .map_err(|e| format!("unable to convert float to decimal: {e}").into()),
        Boolean(v) => Ok(Value::Decimal(if v { Decimal::ONE } else { Decimal::ZERO })),
        Null => Ok(Value::Decimal(Decimal::ZERO)),
        Bytes(v) => {
            let s = String::from_utf8_lossy(&v);
            s.parse::<Decimal>()
                .map(Value::Decimal)
                .map_err(|e| format!("invalid decimal string \"{s}\": {e}").into())
        }
        v => Err(format!("unable to coerce {} into decimal", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToDecimal;

impl Function for ToDecimal {
    fn identifier(&self) -> &'static str {
        "to_decimal"
    }

    fn usage(&self) -> &'static str {
        "Coerces the `value` into a decimal (up to 28-29 significant digits of precision)."
    }

    fn category(&self) -> &'static str {
        Category::Coerce.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::DECIMAL
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to convert to a decimal.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Integer",
                source: "to_decimal(5)",
                result: Ok("d'5'"),
            },
            example! {
                title: "Float",
                source: "to_decimal(5.6)",
                result: Ok("d'5.6'"),
            },
            example! {
                title: "Decimal (passthrough)",
                source: "to_decimal(d'123.456')",
                result: Ok("d'123.456'"),
            },
            example! {
                title: "True",
                source: "to_decimal(true)",
                result: Ok("d'1'"),
            },
            example! {
                title: "False",
                source: "to_decimal(false)",
                result: Ok("d'0'"),
            },
            example! {
                title: "Null",
                source: "to_decimal(null)",
                result: Ok("d'0'"),
            },
            example! {
                title: "Valid string",
                source: "to_decimal!(s'123.456')",
                result: Ok("d'123.456'"),
            },
            example! {
                title: "Invalid string",
                source: "to_decimal!(s'foobar')",
                result: Err(
                    r#"function call error for "to_decimal" at (0:22): invalid decimal string "foobar": Invalid decimal: unknown character"#,
                ),
            },
            example! {
                title: "Array",
                source: "to_decimal!([])",
                result: Err(
                    r#"function call error for "to_decimal" at (0:15): unable to coerce array into decimal"#,
                ),
            },
            example! {
                title: "Object",
                source: "to_decimal!({})",
                result: Err(
                    r#"function call error for "to_decimal" at (0:15): unable to coerce object into decimal"#,
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

        Ok(ToDecimalFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToDecimalFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToDecimalFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        to_decimal(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        TypeDef::decimal().maybe_fallible(
            td.contains_bytes()
                || td.contains_float()
                || td.contains_array()
                || td.contains_object()
                || td.contains_regex()
                || td.contains_timestamp(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    test_function![
        to_decimal => ToDecimal;

        decimal {
            args: func_args![value: Value::Decimal(dec!(20.5))],
            want: Ok(Value::Decimal(dec!(20.5))),
            tdef: TypeDef::decimal().infallible(),
        }

        integer {
            args: func_args![value: 20],
            want: Ok(Value::Decimal(Decimal::from(20))),
            tdef: TypeDef::decimal().infallible(),
        }

        float {
            args: func_args![value: 20.5],
            want: Ok(Value::Decimal(dec!(20.5))),
            tdef: TypeDef::decimal().fallible(),
        }

        // 0.1 + 0.2 in f64 is 0.30000000000000004 — the conversion must
        // preserve this imprecision rather than silently rounding to 0.3.
        float_imprecise {
            args: func_args![value: 0.1_f64 + 0.2_f64],
            want: Ok(Value::Decimal(dec!(0.30000000000000004))),
            tdef: TypeDef::decimal().fallible(),
        }

        string {
            args: func_args![value: "123.456"],
            want: Ok(Value::Decimal(dec!(123.456))),
            tdef: TypeDef::decimal().fallible(),
        }
    ];
}
