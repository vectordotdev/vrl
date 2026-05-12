use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsBoolean;

impl Function for IsBoolean {
    fn identifier(&self) -> &'static str {
        "is_boolean"
    }

    fn usage(&self) -> &'static str {
        "Check if the `value`'s type is a boolean."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is a boolean.",
            "Returns `false` if `value` is anything else.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to check if it is a Boolean.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid boolean",
                source: "is_boolean(false)",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_boolean("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Null",
                source: "is_boolean(null)",
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

        Ok(IsBooleanFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsBooleanFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsBooleanFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_boolean()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_boolean => IsBoolean;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        array {
            args: func_args![value: value!(false)],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
