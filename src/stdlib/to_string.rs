use crate::compiler::prelude::*;

fn to_string(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null, Timestamp};
    use chrono::SecondsFormat;
    let value = match value {
        v @ Bytes(_) => v,
        Integer(v) => v.to_string().into(),
        Float(v) => v.to_string().into(),
        Boolean(v) => v.to_string().into(),
        Timestamp(v) => v.to_rfc3339_opts(SecondsFormat::AutoSi, true).into(),
        Null => "".into(),
        v => return Err(format!("unable to coerce {} into string", v.kind()).into()),
    };
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
pub struct ToString;

impl Function for ToString {
    fn identifier(&self) -> &'static str {
        "to_string"
    }

    fn usage(&self) -> &'static str {
        "Coerces the `value` into a string."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not an integer, float, boolean, string, timestamp, or null."]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "If `value` is an integer or float, returns the string representation.",
            "If `value` is a boolean, returns `\"true\"` or `\"false\"`.",
            "If `value` is a timestamp, returns an [RFC 3339](\\(urls.rfc3339)) representation.",
            "If `value` is a null, returns `\"\"`.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to convert to a string.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Coerce to a string (Boolean)",
                source: "to_string(true)",
                result: Ok("s'true'"),
            },
            example! {
                title: "Coerce to a string (int)",
                source: "to_string(52)",
                result: Ok("s'52'"),
            },
            example! {
                title: "Coerce to a string (float)",
                source: "to_string(52.2)",
                result: Ok("s'52.2'"),
            },
            example! {
                title: "String",
                source: "to_string(s'foo')",
                result: Ok("foo"),
            },
            example! {
                title: "False",
                source: "to_string(false)",
                result: Ok("s'false'"),
            },
            example! {
                title: "Null",
                source: "to_string(null)",
                result: Ok(""),
            },
            example! {
                title: "Timestamp",
                source: "to_string(t'2020-01-01T00:00:00Z')",
                result: Ok("2020-01-01T00:00:00Z"),
            },
            example! {
                title: "Array",
                source: "to_string!([])",
                result: Err(
                    r#"function call error for "to_string" at (0:14): unable to coerce array into string"#,
                ),
            },
            example! {
                title: "Object",
                source: "to_string!({})",
                result: Err(
                    r#"function call error for "to_string" at (0:14): unable to coerce object into string"#,
                ),
            },
            example! {
                title: "Regex",
                source: "to_string!(r'foo')",
                result: Err(
                    r#"function call error for "to_string" at (0:18): unable to coerce regex into string"#,
                ),
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

        Ok(ToStringFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToStringFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToStringFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        to_string(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        TypeDef::bytes()
            .maybe_fallible(td.contains_array() || td.contains_object() || td.contains_regex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        to_string => ToString;

        integer {
            args: func_args![value: 20],
            want: Ok("20"),
            tdef: TypeDef::bytes(),
        }

        float {
            args: func_args![value: 20.5],
            want: Ok("20.5"),
            tdef: TypeDef::bytes(),
        }
    ];
}
