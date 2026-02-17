use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;

fn to_int(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null, Timestamp};

    match value {
        Integer(_) => Ok(value),
        #[allow(clippy::cast_possible_truncation)] //TODO evaluate removal options
        Float(v) => Ok(Integer(v.into_inner() as i64)),
        Boolean(v) => Ok(Integer(i64::from(v))),
        Null => Ok(0.into()),
        Bytes(v) => Conversion::Integer
            .convert(v)
            .map_err(|e| e.to_string().into()),
        Timestamp(v) => Ok(v.timestamp().into()),
        v => Err(format!("unable to coerce {} into integer", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToInt;

impl Function for ToInt {
    fn identifier(&self) -> &'static str {
        "to_int"
    }

    fn usage(&self) -> &'static str {
        "Coerces the `value` into an integer."
    }

    fn category(&self) -> &'static str {
        Category::Coerce.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` is a string but the text is not an integer.",
            "`value` is not a string, int, or timestamp.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "If `value` is an integer, it will be returned as-is.",
            "If `value` is a float, it will be truncated to its integer portion.",
            "If `value` is a string, it must be the string representation of an integer or else an error is raised.",
            "If `value` is a boolean, `0` is returned for `false` and `1` is returned for `true`.",
            "If `value` is a timestamp, a [Unix timestamp](https://en.wikipedia.org/wiki/Unix_time) (in seconds) is returned.",
            "If `value` is null, `0` is returned.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to convert to an integer.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Coerce to an int (string)",
                source: "to_int!(\"2\")",
                result: Ok("2"),
            },
            example! {
                title: "Coerce to an int (timestamp)",
                source: "to_int(t'2020-12-30T22:20:53.824727Z')",
                result: Ok("1609366853"),
            },
            example! {
                title: "Integer",
                source: "to_int(5)",
                result: Ok("5"),
            },
            example! {
                title: "Float",
                source: "to_int(5.6)",
                result: Ok("5"),
            },
            example! {
                title: "True",
                source: "to_int(true)",
                result: Ok("1"),
            },
            example! {
                title: "False",
                source: "to_int(false)",
                result: Ok("0"),
            },
            example! {
                title: "Null",
                source: "to_int(null)",
                result: Ok("0"),
            },
            example! {
                title: "Invalid string",
                source: "to_int!(s'foobar')",
                result: Err(
                    r#"function call error for "to_int" at (0:18): Invalid integer "foobar": invalid digit found in string"#,
                ),
            },
            example! {
                title: "Array",
                source: "to_int!([])",
                result: Err(
                    r#"function call error for "to_int" at (0:11): unable to coerce array into integer"#,
                ),
            },
            example! {
                title: "Object",
                source: "to_int!({})",
                result: Err(
                    r#"function call error for "to_int" at (0:11): unable to coerce object into integer"#,
                ),
            },
            example! {
                title: "Regex",
                source: "to_int!(r'foo')",
                result: Err(
                    r#"function call error for "to_int" at (0:15): unable to coerce regex into integer"#,
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

        Ok(ToIntFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToIntFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        to_int(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        TypeDef::integer().maybe_fallible(
            td.contains_bytes()
                || td.contains_array()
                || td.contains_object()
                || td.contains_regex(),
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;

    test_function![
        to_int => ToInt;

        string {
             args: func_args![value: "20"],
             want: Ok(20),
             tdef: TypeDef::integer().fallible(),
        }

        float {
             args: func_args![value: 20.5],
             want: Ok(20),
             tdef: TypeDef::integer().infallible(),
        }

        timezone {
             args: func_args![value: DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 12:00:00 +0000")
                            .unwrap()
                            .with_timezone(&Utc)],
             want: Ok(1_571_227_200),
             tdef: TypeDef::integer().infallible(),
         }
    ];
}
