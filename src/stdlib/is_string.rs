use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsString;

impl Function for IsString {
    fn identifier(&self) -> &'static str {
        "is_string"
    }

    fn usage(&self) -> &'static str {
        "Check if `value`'s type is a string."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a string.",
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid string",
                source: r#"is_string("a string")"#,
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: "is_string([1, 2, 3])",
                result: Ok("false"),
            },
            example! {
                title: "Boolean",
                source: "is_string(true)",
                result: Ok("false"),
            },
            example! {
                title: "Null",
                source: "is_string(null)",
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

        Ok(IsStringFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsStringFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsStringFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_bytes()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_string => IsString;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        integer {
            args: func_args![value: value!(1789)],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
