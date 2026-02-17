use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsRegex;

impl Function for IsRegex {
    fn identifier(&self) -> &'static str {
        "is_regex"
    }

    fn usage(&self) -> &'static str {
        "Check if `value`'s type is a regex."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is a regex.",
            "Returns `false` if `value` is anything else.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a regex.",
            default: None,
            enum_variants: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid regex",
                source: r"is_regex(r'pattern')",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_regex("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Null value",
                source: "is_regex(null)",
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
