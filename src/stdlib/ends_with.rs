use crate::compiler::prelude::*;
use crate::stdlib::string_utils::convert_to_string;

fn ends_with(value: Value, substring: Value, case_sensitive: bool) -> Resolved {
    let value = convert_to_string(value, case_sensitive)?;
    let substring = convert_to_string(substring, case_sensitive)?;
    Ok(value.ends_with(&substring).into())
}

#[derive(Clone, Copy, Debug)]
pub struct EndsWith;

impl Function for EndsWith {
    fn identifier(&self) -> &'static str {
        "ends_with"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "substring",
                kind: kind::BYTES,
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
        let substring = arguments.required("substring");
        let case_sensitive = arguments.optional("case_sensitive").unwrap_or(expr!(true));

        Ok(EndsWithFn {
            value,
            substring,
            case_sensitive,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "case sensitive",
                source: r#"ends_with("foobar", "R")"#,
                result: Ok("false"),
            },
            Example {
                title: "case insensitive",
                source: r#"ends_with("foobar", "R", false)"#,
                result: Ok("true"),
            },
            Example {
                title: "mismatch",
                source: r#"ends_with("foobar", "foo")"#,
                result: Ok("false"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EndsWithFn {
    value: Box<dyn Expression>,
    substring: Box<dyn Expression>,
    case_sensitive: Box<dyn Expression>,
}

impl FunctionExpression for EndsWithFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let case_sensitive = self.case_sensitive.resolve(ctx)?;
        let case_sensitive = case_sensitive.try_boolean()?;
        let substring = self.substring.resolve(ctx)?;
        let value = self.value.resolve(ctx)?;

        ends_with(value, substring, case_sensitive)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        ends_with => EndsWith;

        no {
            args: func_args![value: "bar",
                             substring: "foo"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        opposite {
            args: func_args![value: "bar",
                             substring: "foobar"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        subset {
            args: func_args![value: "foobar",
                             substring: "oba"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        yes {
            args: func_args![value: "foobar",
                             substring: "bar"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        starts_with {
            args: func_args![value: "foobar",
                             substring: "foo"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        uppercase {
            args: func_args![value: "fooBAR",
                             substring: "BAR"
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_sensitive {
            args: func_args![value: "foobar",
                             substring: "BAR"
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive {
            args: func_args![value: "foobar",
                             substring: "BAR",
                             case_sensitive: false],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
