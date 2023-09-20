use crate::compiler::prelude::*;
use crate::stdlib::string_utils::convert_to_string;

fn contains_all(value: Value, substrings: Value, case_sensitive: bool) -> Resolved {
    let value_string = convert_to_string(value, case_sensitive)?;
    let substring_values = substrings.try_array()?;

    for substring_value in substring_values {
        let substring = convert_to_string(substring_value, case_sensitive)?;
        if !value_string.contains(&substring) {
            return Ok(false.into());
        }
    }
    Ok(true.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ContainsAll;

impl Function for ContainsAll {
    fn identifier(&self) -> &'static str {
        "contains_all"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "substrings",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "case_sensitive",
                kind: kind::BOOLEAN,
                required: false,
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
        let substrings = arguments.required("substrings");
        let case_sensitive = arguments.optional("case_sensitive").unwrap_or(expr!(true));

        Ok(ContainsAllFn {
            value,
            substrings,
            case_sensitive,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "contains_all true",
                source: r#"contains_all("The Needle In The Haystack", ["Needle", "Haystack"])"#,
                result: Ok("true"),
            },
            Example {
                title: "contains_all false",
                source: r#"contains("the NEEDLE in the haystack", "needle", "haystack")"#,
                result: Ok("false"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct ContainsAllFn {
    value: Box<dyn Expression>,
    substrings: Box<dyn Expression>,
    case_sensitive: Box<dyn Expression>,
}

impl FunctionExpression for ContainsAllFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let substrings = self.substrings.resolve(ctx)?;
        let case_sensitive = self.case_sensitive.resolve(ctx)?.try_boolean()?;
        contains_all(value, substrings, case_sensitive)
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        contains_all => ContainsAll;

        no {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["the", "duck"])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        yes {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["The Needle", "Needle In"])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_sensitive_yes {
            args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["Needle", "Haystack"])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

         case_sensitive_no {
                        args: func_args![value: value!("The Needle In The Haystack"),
                             substrings: value!(["needle", "haystack"])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive_no {
                        args: func_args![value: value!("The Needle In The Haystack"),
                                        substrings: value!(["thread", "haystack"]),
                                        case_sensitive: false],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive_yes {
                       args: func_args![value: value!("The Needle In The Haystack"),
                                        substrings: value!(["needle", "haystack"]),
                                        case_sensitive: false],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
