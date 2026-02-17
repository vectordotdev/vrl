use crate::compiler::function::EnumVariant;
use crate::compiler::prelude::*;

use crate::stdlib::casing::{ORIGINAL_CASE, into_case};
use convert_case::Case;

use super::into_boundary;

#[derive(Clone, Copy, Debug)]
pub struct Snakecase;

impl Function for Snakecase {
    fn identifier(&self) -> &'static str {
        "snakecase"
    }

    fn usage(&self) -> &'static str {
        "Takes the `value` string, and turns it into snake_case. Optionally, you can pass in the existing case of the function, or else we will try to figure out the case automatically."
    }

    fn category(&self) -> &'static str {
        Category::String.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
                description: "The string to convert to snake_case.",
                default: None,
                enum_variants: None,
            },
            ORIGINAL_CASE,
            Parameter {
                keyword: "excluded_boundaries",
                kind: kind::ARRAY,
                required: false,
                description: "Case boundaries to exclude during conversion.",
                default: None,
                enum_variants: Some(&[
                    EnumVariant {
                        value: "lower_upper",
                        description: "Lowercase to uppercase transitions (e.g., 'camelCase' → 'camel' + 'case')",
                    },
                    EnumVariant {
                        value: "upper_lower",
                        description: "Uppercase to lowercase transitions (e.g., 'CamelCase' → 'Camel' + 'Case')",
                    },
                    EnumVariant {
                        value: "acronym",
                        description: "Acronyms from words (e.g., 'XMLHttpRequest' → 'xmlhttp' + 'request')",
                    },
                    EnumVariant {
                        value: "lower_digit",
                        description: "Lowercase to digit transitions (e.g., 'foo2bar' → 'foo2_bar')",
                    },
                    EnumVariant {
                        value: "upper_digit",
                        description: "Uppercase to digit transitions (e.g., 'versionV2' → 'version_v2')",
                    },
                    EnumVariant {
                        value: "digit_lower",
                        description: "Digit to lowercase transitions (e.g., 'Foo123barBaz' → 'foo' + '123bar' + 'baz')",
                    },
                    EnumVariant {
                        value: "digit_upper",
                        description: "Digit to uppercase transitions (e.g., 'Version123Test' → 'version' + '123test')",
                    },
                ]),
            },
        ];
        PARAMETERS
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

        let excluded_boundaries = arguments
            .optional_array("excluded_boundaries")?
            .map(|arr| {
                let mut boundaries = Vec::new();
                for expr in arr {
                    let value = expr.resolve_constant(state).ok_or_else(
                        || -> Box<dyn DiagnosticMessage> {
                            Box::new(ExpressionError::from(
                                "expected static string for excluded_boundaries",
                            ))
                        },
                    )?;
                    let boundary = into_boundary(
                        value
                            .try_bytes_utf8_lossy()
                            .expect("cant convert to string")
                            .as_ref(),
                    )?;
                    boundaries.push(boundary);
                }
                Ok::<_, Box<dyn DiagnosticMessage>>(boundaries)
            })
            .transpose()?;

        Ok(SnakecaseFn {
            value,
            original_case,
            excluded_boundaries,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "snake_case a string",
                source: r#"snakecase("input-string")"#,
                result: Ok("input_string"),
            },
            example! {
                title: "snake_case a string with original case",
                source: r#"snakecase("input-string", original_case: "kebab-case")"#,
                result: Ok("input_string"),
            },
            example! {
                title: "snake_case with excluded boundaries",
                source: r#"snakecase("s3BucketDetails", excluded_boundaries: ["digit_lower", "lower_digit", "upper_digit"])"#,
                result: Ok("s3_bucket_details"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct SnakecaseFn {
    value: Box<dyn Expression>,
    original_case: Option<Case>,
    excluded_boundaries: Option<Vec<convert_case::Boundary>>,
}

impl FunctionExpression for SnakecaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string_value = value
            .try_bytes_utf8_lossy()
            .expect("can't convert to string");

        match &self.excluded_boundaries {
            Some(boundaries) if !boundaries.is_empty() => {
                Ok(super::convert_case_with_excluded_boundaries(
                    &string_value,
                    Case::Snake,
                    self.original_case,
                    boundaries.as_slice(),
                ))
            }
            _ => super::convert_case(&value, Case::Snake, self.original_case),
        }
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
        snakecase => Snakecase;

        simple {
            args: func_args![value: value!("camelCase"), original_case: "camelCase"],
            want: Ok(value!("camel_case")),
            tdef: TypeDef::bytes(),
        }

        no_case {
            args: func_args![value: value!("camelCase")],
            want: Ok(value!("camel_case")),
            tdef: TypeDef::bytes(),
        }

        with_empty_excluded_boundary {
            args: func_args![value: value!("camelCase"), excluded_boundaries: value!([])],
            want: Ok(value!("camel_case")),
            tdef: TypeDef::bytes(),
        }

        with_lower_upper_excluded {
            args: func_args![value: value!("camelCase"), excluded_boundaries: value!(["lower_upper"])],
            want: Ok(value!("camelcase")),
            tdef: TypeDef::bytes(),
        }

        with_s3_bucket_details {
            args: func_args![value: value!("s3BucketDetails")],
            want: Ok(value!("s_3_bucket_details")),
            tdef: TypeDef::bytes(),
        }

        with_s3_bucket_details_exclude_acronym {
            args: func_args![value: value!("s3BucketDetails"), excluded_boundaries: value!(["digit_lower", "lower_digit", "upper_digit"])],
            want: Ok(value!("s3_bucket_details")),
            tdef: TypeDef::bytes(),
        }
    ];
}
