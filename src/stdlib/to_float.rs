use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;

pub(crate) fn bytes_to_float(bytes: Bytes) -> Resolved {
    Conversion::Float
        .convert(bytes)
        .map_err(|e| e.to_string().into())
}

#[allow(clippy::cast_precision_loss)] //TODO evaluate removal options
fn to_float(value: Value) -> Resolved {
    use Value::{Boolean, Bytes, Float, Integer, Null, Timestamp};
    match value {
        Float(_) => Ok(value),
        Integer(v) => Ok(Value::from_f64_or_zero(v as f64)),
        Boolean(v) => Ok(NotNan::new(if v { 1.0 } else { 0.0 }).unwrap().into()),
        Null => Ok(NotNan::new(0.0).unwrap().into()),
        Timestamp(v) => {
            let nanoseconds = match v.timestamp_nanos_opt() {
                Some(nanos) => nanos as f64,
                None => return Err(ValueError::OutOfRange(Kind::timestamp()).into()),
            };
            Ok(Value::from_f64_or_zero(nanoseconds / 1_000_000_000_f64))
        }
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

    fn usage(&self) -> &'static str {
        "Coerces the `value` into a float."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a supported float representation."]
    }

    fn return_kind(&self) -> u16 {
        kind::FLOAT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "If `value` is a float, it will be returned as-is.",
            "If `value` is an integer, it will be returned as as a float.",
            "If `value` is a string, it must be the string representation of an float or else an error is raised.",
            "If `value` is a boolean, `0.0` is returned for `false` and `1.0` is returned for `true`.",
            "If `value` is a timestamp, a [Unix timestamp](https://en.wikipedia.org/wiki/Unix_time) with fractional seconds is returned.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to convert to a float. Must be convertible to a float, otherwise an error is raised.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Coerce to a float",
                source: "to_float!(\"3.145\")",
                result: Ok("3.145"),
            },
            example! {
                title: "Coerce to a float (timestamp)",
                source: "to_float(t'2020-12-30T22:20:53.824727Z')",
                result: Ok("1609366853.824727"),
            },
            example! {
                title: "Integer",
                source: "to_float(5)",
                result: Ok("5.0"),
            },
            example! {
                title: "Float",
                source: "to_float(5.6)",
                result: Ok("5.6"),
            },
            example! {
                title: "True",
                source: "to_float(true)",
                result: Ok("1.0"),
            },
            example! {
                title: "False",
                source: "to_float(false)",
                result: Ok("0.0"),
            },
            example! {
                title: "Null",
                source: "to_float(null)",
                result: Ok("0.0"),
            },
            example! {
                title: "Invalid string",
                source: "to_float!(s'foobar')",
                result: Err(
                    r#"function call error for "to_float" at (0:20): Invalid floating point number "foobar": invalid float literal"#,
                ),
            },
            example! {
                title: "Array",
                source: "to_float!([])",
                result: Err(
                    r#"function call error for "to_float" at (0:13): unable to coerce array into float"#,
                ),
            },
            example! {
                title: "Object",
                source: "to_float!({})",
                result: Err(
                    r#"function call error for "to_float" at (0:13): unable to coerce object into float"#,
                ),
            },
            example! {
                title: "Regex",
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
             args: func_args![value: Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap().with_nanosecond(12_000_000).unwrap()],

             want: Ok(1_404_810_611.012),
             tdef: TypeDef::float().infallible(),
        }
    ];
}
