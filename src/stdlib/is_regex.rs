use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsRegex;

impl Function for IsRegex {
    fn identifier(&self) -> &'static str {
        "is_regex"
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
                source: r#"is_regex("foobar")"#,
                result: Ok("false"),
            },
            Example {
                title: "regex",
                source: r"is_regex(r'\d+')",
                result: Ok("true"),
            },
            Example {
                title: "null",
                source: r#"is_regex(null)"#,
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

        Ok(IsRegexFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsRegexFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsRegexFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_regex()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    test_function![
        is_regex => IsRegex;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        regex {
            args: func_args![value: value!(Regex::new(r"\d+").unwrap())],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
