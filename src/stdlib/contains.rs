use crate::compiler::prelude::*;
use crate::stdlib::string_utils::convert_to_string;
use std::sync::LazyLock;

static DEFAULT_CASE_SENSITIVE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The text to search.",
            default: None,
        },
        Parameter {
            keyword: "substring",
            kind: kind::BYTES,
            required: true,
            description: "The substring to search for in `value`.",
            default: None,
        },
        Parameter {
            keyword: "case_sensitive",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether the match should be case sensitive.",
            default: Some(&DEFAULT_CASE_SENSITIVE),
        },
    ]
});

fn contains(value: &Value, substring: &Value, case_sensitive: Value) -> Resolved {
    let case_sensitive = case_sensitive.try_boolean()?;
    let value = convert_to_string(value, !case_sensitive)?;
    let substring = convert_to_string(substring, !case_sensitive)?;
    Ok(value.contains(substring.as_ref()).into())
}

#[derive(Clone, Copy, Debug)]
pub struct Contains;

impl Function for Contains {
    fn identifier(&self) -> &'static str {
        "contains"
    }

    fn usage(&self) -> &'static str {
        "Determines whether the `value` string contains the specified `substring`."
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let substring = arguments.required("substring");
        let case_sensitive = arguments.optional("case_sensitive");

        Ok(ContainsFn {
            value,
            substring,
            case_sensitive,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "String contains with default parameters (case sensitive)",
                source: r#"contains("banana", "AnA")"#,
                result: Ok("false"),
            },
            example! {
                title: "String contains (case insensitive)",
                source: r#"contains("banana", "AnA", case_sensitive: false)"#,
                result: Ok("true"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct ContainsFn {
    value: Box<dyn Expression>,
    substring: Box<dyn Expression>,
    case_sensitive: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ContainsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let substring = self.substring.resolve(ctx)?;
        let case_sensitive = self
            .case_sensitive
            .map_resolve_with_default(ctx, || DEFAULT_CASE_SENSITIVE.clone())?;

        contains(&value, &substring, case_sensitive)
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
        contains => Contains;

        no {
            args: func_args![value: value!("foo"),
                             substring: value!("bar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        yes {
            args: func_args![value: value!("foobar"),
                             substring: value!("foo")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        entirely {
            args: func_args![value: value!("foo"),
                             substring: value!("foo")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        middle {
            args: func_args![value: value!("foobar"),
                             substring: value!("oba")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        start {
            args: func_args![value: value!("foobar"),
                             substring: value!("foo")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        end {
            args: func_args![value: value!("foobar"),
                             substring: value!("bar")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_sensitive_yes {
            args: func_args![value: value!("fooBAR"),
                             substring: value!("BAR"),
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

         case_sensitive_yes_lowercase {
            args: func_args![value: value!("fooBAR"),
                             substring: value!("bar"),
                             case_sensitive: true
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_sensitive_no_uppercase {
            args: func_args![value: value!("foobar"),
                             substring: value!("BAR"),
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        case_insensitive_yes_uppercase {
            args: func_args![value: value!("foobar"),
                             substring: value!("BAR"),
                             case_sensitive: false
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
