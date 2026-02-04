use crate::compiler::prelude::*;

fn r#mod(value: Value, modulus: Value) -> Resolved {
    let result = value.try_rem(modulus)?;
    Ok(result)
}

#[derive(Clone, Copy, Debug)]
pub struct Mod;

impl Function for Mod {
    fn identifier(&self) -> &'static str {
        "mod"
    }

    fn usage(&self) -> &'static str {
        "Calculates the remainder of `value` divided by `modulus`."
    }

    fn category(&self) -> &'static str {
        Category::Number.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` is not an integer, float, or decimal.",
            "`modulus` is not an integer, float, or decimal.",
            "`modulus` is equal to 0.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER | kind::FLOAT
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required(
                "value",
                kind::INTEGER | kind::FLOAT | kind::DECIMAL,
                "The value the `modulus` is applied to.",
            ),
            Parameter::required(
                "modulus",
                kind::INTEGER | kind::FLOAT | kind::DECIMAL,
                "The `modulus` value.",
            ),
        ];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Calculate the remainder of two integers",
                source: "mod(5, 2)",
                result: Ok("1"),
            },
            example! {
                title: "Calculate the remainder of two decimals",
                source: "mod(d'5.5', d'2')",
                result: Ok("d'1.5'"),
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
        let modulus = arguments.required("modulus");
        // TODO: return a compile-time error if modulus is 0

        Ok(ModFn { value, modulus }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ModFn {
    value: Box<dyn Expression>,
    modulus: Box<dyn Expression>,
}

impl FunctionExpression for ModFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let modulus = self.modulus.resolve(ctx)?;
        r#mod(value, modulus)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let value_def = self.value.type_def(state);
        let modulus_def = self.modulus.type_def(state);

        // Decimal % Float or Float % Decimal -> compile-time error
        if (value_def.is_decimal() && modulus_def.is_float())
            || (value_def.is_float() && modulus_def.is_decimal())
        {
            return value_def
                .fallible()
                .union(modulus_def.fallible())
                .with_kind(Kind::never());
        }

        // Division is infallible if the rhs is a literal normal float, a literal non-zero integer,
        // or a literal non-zero decimal.
        match self.modulus.resolve_constant(state) {
            Some(value) if value.is_float() || value.is_integer() || value.is_decimal() => {
                match value {
                    Value::Float(v) if v.is_normal() => TypeDef::float().infallible(),
                    Value::Float(_) => TypeDef::float().fallible(),
                    Value::Integer(v) if v != 0 => TypeDef::integer().infallible(),
                    Value::Integer(_) => TypeDef::integer().fallible(),
                    Value::Decimal(v) if !v.is_zero() => TypeDef::decimal().infallible(),
                    Value::Decimal(_) => TypeDef::decimal().fallible(),
                    _ => TypeDef::float().or_integer().or_decimal().fallible(),
                }
            }
            _ => TypeDef::float().or_integer().or_decimal().fallible(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use rust_decimal::dec;

    test_function![
        r#mod => Mod;

        int_mod {
            args: func_args![value: 5, modulus: 2],
            want: Ok(value!(1)),
            tdef: TypeDef::integer().infallible(),
        }

        float_mod {
            args: func_args![value: 5.0, modulus: 2.0],
            want: Ok(value!(1.0)),
            tdef: TypeDef::float().infallible(),
        }

        decimal_mod {
            args: func_args![value: Value::Decimal(dec!(5.5)), modulus: Value::Decimal(dec!(2))],
            want: Ok(Value::Decimal(dec!(1.5))),
            tdef: TypeDef::decimal().infallible(),
        }

        fallible_mod {
            args: func_args![value: 5.0, modulus: {}],
            want: Err("can't calculate remainder of type float and null"),
            tdef: TypeDef::float().or_integer().or_decimal().fallible(),
        }
    ];
}
