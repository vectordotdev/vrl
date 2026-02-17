use crate::compiler::prelude::*;

use crate::stdlib::casing::into_case;
use convert_case::Case;

#[derive(Clone, Copy, Debug)]
pub struct Pascalcase;

impl Function for Pascalcase {
    fn identifier(&self) -> &'static str {
        "pascalcase"
    }

    fn usage(&self) -> &'static str {
        "Takes the `value` string, and turns it into PascalCase. Optionally, you can pass in the existing case of the function, or else we will try to figure out the case automatically."
    }

    fn category(&self) -> &'static str {
        Category::String.as_ref()
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
                description: "The string to convert to PascalCase.",
                default: None,
                enum_variants: None,
            },
            Parameter {
                keyword: "original_case",
                kind: kind::BYTES,
                required: false,
                description: "Optional hint on the original case type. Must be one of: kebab-case, camelCase, PascalCase, SCREAMING_SNAKE, snake_case",
                default: None,
                enum_variants: None,
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

        Ok(PascalcaseFn {
            value,
            original_case,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "PascalCase a string without specifying original case",
                source: r#"pascalcase("input-string")"#,
                result: Ok("InputString"),
            },
            example! {
                title: "PascalCase a snake_case string",
                source: r#"pascalcase("foo_bar_baz", "snake_case")"#,
                result: Ok("FooBarBaz"),
            },
            example! {
                title: "PascalCase specifying the wrong original case (only capitalizes)",
                source: r#"pascalcase("foo_bar_baz", "kebab-case")"#,
                result: Ok("Foo_bar_baz"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct PascalcaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
}

impl FunctionExpression for PascalcaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        super::convert_case(&value, Case::Pascal, self.original_case)
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
        pascalcase => Pascalcase;

        simple {
            args: func_args![value: value!("input_string"), original_case: "snake_case"],
            want: Ok(value!("InputString")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("input_string")],
            want: Ok(value!("InputString")),
            tdef: TypeDef::bytes(),
        }
    ];
}
