use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;

fn to_float(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null, Timestamp};
    match value {
        Float(_) => Ok(value),
        Integer(v) => Ok(Value::from_f64_or_zero(v as f64)),
        Boolean(v) => Ok(NotNan::new(if v { 1.0 } else { 0.0 }).unwrap().into()),
        Null => Ok(NotNan::new(0.0).unwrap().into()),
        Timestamp(v) => Ok(Value::from_f64_or_zero(
            v.timestamp_nanos() as f64 / 1_000_000_000_f64,
        )),
        Bytes(v) => Conversion::Float
            .convert(v)
            .map_err(|e| e.to_string().into()),
        v => Err(format!("unable to coerce {} into float", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ToFloat;

impl Function for ToFloat {
    fn identifier(&self) -> &'static str {
        "to_float"
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
                title: "integer",
                source: "to_float(5)",
                result: Ok("5.0"),
            },
            Example {
                title: "float",
                source: "to_float(5.6)",
                result: Ok("5.6"),
            },
            Example {
                title: "true",
                source: "to_float(true)",
                result: Ok("1.0"),
            },
            Example {
                title: "false",
                source: "to_float(false)",
                result: Ok("0.0"),
            },
            Example {
                title: "null",
                source: "to_float(null)",
                result: Ok("0.0"),
            },
            Example {
                title: "valid string",
                source: "to_float!(s'5.6')",
                result: Ok("5.6"),
            },
            Example {
                title: "invalid string",
                source: "to_float!(s'foobar')",
                result: Err(
                    r#"function call error for "to_float" at (0:20): Invalid floating point number "foobar": invalid float literal"#,
                ),
            },
            Example {
                title: "timestamp",
                source: "to_float(t'2020-01-01T00:00:00.100Z')",
                result: Ok("1577836800.1"),
            },
            Example {
                title: "array",
                source: "to_float!([])",
                result: Err(
                    r#"function call error for "to_float" at (0:13): unable to coerce array into float"#,
                ),
            },
            Example {
                title: "object",
                source: "to_float!({})",
                result: Err(
                    r#"function call error for "to_float" at (0:13): unable to coerce object into float"#,
                ),
            },
            Example {
                title: "regex",
                source: "to_float!(r'foo')",
                result: Err(
                    r#"function call error for "to_float" at (0:17): unable to coerce regex into float"#,
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

        Ok(ToFloatFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ToFloatFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ToFloatFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        to_float(value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = self.value.type_def(state);

        TypeDef::float().maybe_fallible(
            td.contains_bytes()
                || td.contains_array()
                || td.contains_object()
                || td.contains_regex(),
        )
    }
}

#[cfg(test)]
mod tests {
    use chrono::prelude::*;

    use super::*;

    test_function![
        to_float => ToFloat;

        float {
            args: func_args![value: 20.5],
            want: Ok(20.5),
            tdef: TypeDef::float().infallible(),
        }

        integer {
            args: func_args![value: 20],
            want: Ok(20.0),
            tdef: TypeDef::float().infallible(),
        }

        timestamp {
             args: func_args![value: Utc.ymd(2014, 7, 8).and_hms_milli(9, 10, 11, 12)],
             want: Ok(1_404_810_611.012),
             tdef: TypeDef::float().infallible(),
        }
    ];
}
