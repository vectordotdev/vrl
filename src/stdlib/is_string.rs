use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsString;

impl Function for IsString {
    fn identifier(&self) -> &'static str {
        "is_string"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "string",
                source: r#"is_string("foobar")"#,
                result: Ok("true"),
            },
            Example {
                title: "boolean",
                source: r#"is_string(true)"#,
                result: Ok("false"),
            },
            Example {
                title: "null",
                source: r#"is_string(null)"#,
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
