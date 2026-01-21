use crate::compiler::prelude::*;

use crate::stdlib::casing::into_case;
use convert_case::Case;

#[derive(Clone, Copy, Debug)]
pub struct Kebabcase;

impl Function for Kebabcase {
    fn identifier(&self) -> &'static str {
        "kebabcase"
    }

    fn usage(&self) -> &'static str {
        "Takes the `value` string, and turns it into kebab-case. Optionally, you can pass in the existing case of the function, or else we will try to figure out the case automatically."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
                description: "The string to convert to kebab-case.",
            },
            Parameter {
                keyword: "original_case",
                kind: kind::BYTES,
                required: false,
                description: "Optional hint on the original case type. Must be one of: kebab-case, camelCase, PascalCase, SCREAMING_SNAKE, snake_case",
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

        Ok(KebabcaseFn {
            value,
            original_case,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "kebab-case a string without specifying original case",
                source: r#"kebabcase("InputString")"#,
                result: Ok("input-string"),
            },
            example! {
                title: "kebab-case a snake_case string",
                source: r#"kebabcase("foo_bar_baz", "snake_case")"#,
                result: Ok("foo-bar-baz"),
            },
            example! {
                title: "kebab-case specifying the wrong original case (noop)",
                source: r#"kebabcase("foo_bar_baz", "PascalCase")"#,
                result: Ok("foo_bar_baz"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct KebabcaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
}

impl FunctionExpression for KebabcaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        super::convert_case(&value, Case::Kebab, self.original_case)
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
        kebabcase => Kebabcase;

        simple {
            args: func_args![value: value!("input_string"), original_case: "snake_case"],
            want: Ok(value!("input-string")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("input_string")],
            want: Ok(value!("input-string")),
            tdef: TypeDef::bytes(),
        }
    ];
}
