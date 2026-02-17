use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsInteger;

impl Function for IsInteger {
    fn identifier(&self) -> &'static str {
        "is_integer"
    }

    fn usage(&self) -> &'static str {
        "Check if the value`'s type is an integer."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is an integer.",
            "Returns `false` if `value` is anything else.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to check if it is an integer.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid integer",
                source: "is_integer(1)",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_integer("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Null",
                source: "is_integer(null)",
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

        Ok(IsIntegerFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsIntegerFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsIntegerFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_integer()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_integer => IsInteger;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        integer {
            args: func_args![value: value!(1789)],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
