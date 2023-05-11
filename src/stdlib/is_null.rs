use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsNull;

impl Function for IsNull {
    fn identifier(&self) -> &'static str {
        "is_null"
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
                title: "array",
                source: r#"is_null([1, 2, 3])"#,
                result: Ok("false"),
            },
            Example {
                title: "string",
                source: r#"is_null("foobar")"#,
                result: Ok("false"),
            },
            Example {
                title: "null",
                source: r#"is_null(null)"#,
                result: Ok("true"),
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

        Ok(IsNullFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsNullFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsNullFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_null()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_null => IsNull;

        array {
            args: func_args![value: value!(null)],
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
