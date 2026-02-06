use crate::compiler::prelude::*;

use crate::stdlib::casing::into_case;
use convert_case::Case;

#[derive(Clone, Copy, Debug)]
pub struct ScreamingSnakecase;

impl Function for ScreamingSnakecase {
    fn identifier(&self) -> &'static str {
        "screamingsnakecase"
    }

    fn usage(&self) -> &'static str {
        "Takes the `value` string, and turns it into SCREAMING_SNAKE case. Optionally, you can pass in the existing case of the function, or else we will try to figure out the case automatically."
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
                description: "The string to convert to SCREAMING_SNAKE case.",
                default: None,
            },
            Parameter {
                keyword: "original_case",
                kind: kind::BYTES,
                required: false,
                description: "Optional hint on the original case type. Must be one of: kebab-case, camelCase, PascalCase, SCREAMING_SNAKE, snake_case",
                default: None,
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let original_case = arguments
            .optional_enum("original_case", &super::variants(), state)?
            .map(|b| {
                into_case(
                    b.try_bytes_utf8_lossy()
                        .expect("cant convert to string")
                        .as_ref(),
                )
            })
            .transpose()?;

        Ok(ScreamingSnakecaseFn {
            value,
            original_case,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "SCREAMING_SNAKE_CASE a string without specifying original case",
                source: r#"screamingsnakecase("input-string")"#,
                result: Ok("INPUT_STRING"),
            },
            example! {
                title: "SCREAMING_SNAKE_CASE a snake_case string",
                source: r#"screamingsnakecase("foo_bar_baz", "snake_case")"#,
                result: Ok("FOO_BAR_BAZ"),
            },
            example! {
                title: "SCREAMING_SNAKE_CASE specifying the wrong original case (capitalizes but doesn't include `_` properly)",
                source: r#"screamingsnakecase("FooBarBaz", "kebab-case")"#,
                result: Ok("FOOBARBAZ"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ScreamingSnakecaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
}

impl FunctionExpression for ScreamingSnakecaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        super::convert_case(&value, Case::Constant, self.original_case)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        screamingsnakecase => ScreamingSnakecase;

        simple {
            args: func_args![value: value!("input_string"), original_case: "snake_case"],
            want: Ok(value!("INPUT_STRING")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("input_string")],
            want: Ok(value!("INPUT_STRING")),
            tdef: TypeDef::bytes(),
        }
    ];
}
