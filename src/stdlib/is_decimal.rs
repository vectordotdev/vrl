use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsDecimal;

impl Function for IsDecimal {
    fn identifier(&self) -> &'static str {
        "is_decimal"
    }

    fn usage(&self) -> &'static str {
        "Check if the value's type is a decimal."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to check if it is a decimal.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid decimal",
                source: "is_decimal(d'123.456')",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type (string)",
                source: r#"is_decimal("foobar")"#,
                result: Ok("false"),
            },
            example! {
                title: "Non-matching type (integer)",
                source: "is_decimal(1515)",
                result: Ok("false"),
            },
            example! {
                title: "Null",
                source: "is_decimal(null)",
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

        Ok(IsDecimalFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsDecimalFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsDecimalFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_decimal()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    test_function![
        is_decimal => IsDecimal;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        integer {
            args: func_args![value: value!(1789)],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        decimal {
            args: func_args![value: Value::Decimal(dec!(123.456))],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
