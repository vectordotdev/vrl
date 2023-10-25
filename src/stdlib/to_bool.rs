use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;

fn to_bool(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null};

    match value {
        Boolean(_) => Ok(value),
        Integer(v) => Ok(Boolean(v != 0)),
        Float(v) => Ok(Boolean(v != 0.0)),
        Null => Ok(Boolean(false)),
        Bytes(v) => Conversion::Boolean
            .convert(v)
            .map_err(|e| e.to_string().into()),
        v => Err(format!("unable to coerce {} into boolean", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToBool;

impl Function for ToBool {
    fn identifier(&self) -> &'static str {
        "to_bool"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "integer (0)",
                source: "to_bool(0)",
                result: Ok("false"),
            },
            Example {
                title: "integer (other)",
                source: "to_bool(2)",
                result: Ok("true"),
            },
            Example {
                title: "float (0)",
                source: "to_bool(0.0)",
                result: Ok("false"),
            },
            Example {
                title: "float (other)",
                source: "to_bool(5.6)",
                result: Ok("true"),
            },
            Example {
                title: "true",
                source: "to_bool(true)",
                result: Ok("true"),
            },
            Example {
                title: "false",
                source: "to_bool(false)",
                result: Ok("false"),
            },
            Example {
                title: "null",
                source: "to_bool(null)",
                result: Ok("false"),
            },
            Example {
                title: "true string",
                source: "to_bool!(s'true')",
                result: Ok("true"),
            },
            Example {
                title: "yes string",
                source: "to_bool!(s'yes')",
                result: Ok("true"),
            },
            Example {
                title: "y string",
                source: "to_bool!(s'y')",
                result: Ok("true"),
            },
            Example {
                title: "non-zero integer string",
                source: "to_bool!(s'1')",
                result: Ok("true"),
            },
            Example {
                title: "false string",
                source: "to_bool!(s'false')",
                result: Ok("false"),
            },
            Example {
                title: "no string",
                source: "to_bool!(s'no')",
                result: Ok("false"),
            },
            Example {
                title: "n string",
                source: "to_bool!(s'n')",
                result: Ok("false"),
            },
            Example {
                title: "zero integer string",
                source: "to_bool!(s'0')",
                result: Ok("false"),
            },
            Example {
                title: "invalid string",
                source: "to_bool!(s'foobar')",
                result: Err(
                    r#"function call error for "to_bool" at (0:19): Invalid boolean value "foobar""#,
                ),
            },
            Example {
                title: "timestamp",
                source: "to_bool!(t'2020-01-01T00:00:00Z')",
                result: Err(
                    r#"function call error for "to_bool" at (0:33): unable to coerce timestamp into boolean"#,
                ),
            },
            Example {
                title: "array",
                source: "to_bool!([])",
                result: Err(
                    r#"function call error for "to_bool" at (0:12): unable to coerce array into boolean"#,
                ),
            },
            Example {
                title: "object",
                source: "to_bool!({})",
                result: Err(
                    r#"function call error for "to_bool" at (0:12): unable to coerce object into boolean"#,
                ),
            },
            Example {
                title: "regex",
                source: "to_bool!(r'foo')",
                result: Err(
                    r#"function call error for "to_bool" at (0:16): unable to coerce regex into boolean"#,
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

        Ok(ToBoolFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToBoolFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToBoolFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        to_bool(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        TypeDef::boolean().maybe_fallible(
            td.contains_bytes()
                || td.contains_timestamp()
                || td.contains_array()
                || td.contains_object()
                || td.contains_regex(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        to_bool => ToBool;

        string_true {
            args: func_args![value: "true"],
            want: Ok(true),
            tdef: TypeDef::boolean().fallible(),
        }

        string_false {
            args: func_args![value: "no"],
            want: Ok(false),
            tdef: TypeDef::boolean().fallible(),
        }

        string_error {
            args: func_args![value: "cabbage"],
            want: Err(r#"Invalid boolean value "cabbage""#),
            tdef: TypeDef::boolean().fallible(),
        }

        number_true {
            args: func_args![value: 20],
            want: Ok(true),
            tdef: TypeDef::boolean().infallible(),
        }

        number_false {
            args: func_args![value: 0],
            want: Ok(false),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
