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

    fn usage(&self) -> &'static str {
        "Coerces the `value` into a boolean."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to convert to a Boolean.",
        }]
    }

    #[allow(clippy::too_many_lines)]
    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Coerce to a Boolean (string)",
                source: "to_bool!(\"yes\")",
                result: Ok("true"),
            },
            example! {
                title: "Coerce to a Boolean (float)",
                source: "to_bool(0.0)",
                result: Ok("false"),
            },
            example! {
                title: "Coerce to a Boolean (int)",
                source: "to_bool(0)",
                result: Ok("false"),
            },
            example! {
                title: "Coerce to a Boolean (null)",
                source: "to_bool(null)",
                result: Ok("false"),
            },
            example! {
                title: "Coerce to a Boolean (Boolean)",
                source: "to_bool(true)",
                result: Ok("true"),
            },
            example! {
                title: "Integer (other)",
                source: "to_bool(2)",
                result: Ok("true"),
            },
            example! {
                title: "Float (other)",
                source: "to_bool(5.6)",
                result: Ok("true"),
            },
            example! {
                title: "False",
                source: "to_bool(false)",
                result: Ok("false"),
            },
            example! {
                title: "True string",
                source: "to_bool!(s'true')",
                result: Ok("true"),
            },
            example! {
                title: "Y string",
                source: "to_bool!(s'y')",
                result: Ok("true"),
            },
            example! {
                title: "Non-zero integer string",
                source: "to_bool!(s'1')",
                result: Ok("true"),
            },
            example! {
                title: "False string",
                source: "to_bool!(s'false')",
                result: Ok("false"),
            },
            example! {
                title: "No string",
                source: "to_bool!(s'no')",
                result: Ok("false"),
            },
            example! {
                title: "N string",
                source: "to_bool!(s'n')",
                result: Ok("false"),
            },
            example! {
                title: "Invalid string",
                source: "to_bool!(s'foobar')",
                result: Err(
                    r#"function call error for "to_bool" at (0:19): Invalid boolean value "foobar""#,
                ),
            },
            example! {
                title: "Timestamp",
                source: "to_bool!(t'2020-01-01T00:00:00Z')",
                result: Err(
                    r#"function call error for "to_bool" at (0:33): unable to coerce timestamp into boolean"#,
                ),
            },
            example! {
                title: "Array",
                source: "to_bool!([])",
                result: Err(
                    r#"function call error for "to_bool" at (0:12): unable to coerce array into boolean"#,
                ),
            },
            example! {
                title: "Object",
                source: "to_bool!({})",
                result: Err(
                    r#"function call error for "to_bool" at (0:12): unable to coerce object into boolean"#,
                ),
            },
            example! {
                title: "Regex",
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
