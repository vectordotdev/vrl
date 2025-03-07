use crate::compiler::prelude::*;

use crate::stdlib::casing::into_case;
use convert_case::Case;

#[derive(Clone, Copy, Debug)]
pub struct Snakecase;

impl Function for Snakecase {
    fn identifier(&self) -> &'static str {
        "snakecase"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "original_case",
                kind: kind::BYTES,
                required: false,
            },
            Parameter {
                keyword: "excluded_boundaries",
                kind: kind::ARRAY,
                required: false,
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

        let excluded_boundaries = match arguments.optional_array("excluded_boundaries") {
            Ok(Some(array_exprs)) => {
                let mut boundaries = Vec::new();

                for expr in array_exprs {
                    if let Some(value) = expr.resolve_constant(state) {
                        if let Some(s) = value.as_str() {
                            match super::into_boundary(s.as_ref()) {
                                Ok(boundary) => boundaries.push(boundary),
                                Err(e) => return Err(e),
                            }
                        } else {
                            return Err(Box::new(ExpressionError::from(
                                "excluded_boundaries must contain only strings",
                            ))
                                as Box<dyn DiagnosticMessage>);
                        }
                    } else {
                        return Err(Box::new(ExpressionError::from(
                            "excluded_boundaries must contain only constant values",
                        )) as Box<dyn DiagnosticMessage>);
                    }
                }

                if boundaries.is_empty() {
                    None
                } else {
                    Some(boundaries)
                }
            }
            Ok(None) => None,
            Err(e) => return Err(Box::new(e) as Box<dyn DiagnosticMessage>),
        };

        Ok(SnakecaseFn {
            value,
            original_case,
            excluded_boundaries,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "snakecase",
                source: r#"snakecase("InputString")"#,
                result: Ok("input_string"),
            },
            Example {
                title: "snakecase with original case",
                source: r#"snakecase("camelCaseInput", original_case: "camelCase")"#,
                result: Ok("camel_case_input"),
            },
            Example {
                title: "snakecase with excluded boundaries",
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
        super::convert_case(&value, Case::Snake, self.original_case, self.excluded_boundaries.clone())
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
