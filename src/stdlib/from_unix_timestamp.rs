use crate::compiler::prelude::*;
use chrono::{TimeZone as _, Utc};
use std::str::FromStr;

fn from_unix_timestamp(value: Value, unit: Unit) -> Resolved {
    use Value::Integer;

    let value = match value {
        Integer(v) => match unit {
            Unit::Seconds => {
                let t = Utc.timestamp_opt(v, 0).single();
                match t {
                    Some(time) => time.into(),
                    None => return Err(format!("unable to coerce {v} into timestamp").into()),
                }
            }
            Unit::Milliseconds => {
                let t = Utc.timestamp_millis_opt(v).single();
                match t {
                    Some(time) => time.into(),
                    None => return Err(format!("unable to coerce {v} into timestamp").into()),
                }
            }
            Unit::Nanoseconds => Utc.timestamp_nanos(v).into(),
        },
        v => return Err(format!("unable to coerce {} into timestamp", v.kind()).into()),
    };
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
pub struct FromUnixTimestamp;

impl Function for FromUnixTimestamp {
    fn identifier(&self) -> &'static str {
        "from_unix_timestamp"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::INTEGER,
                required: true,
            },
            Parameter {
                keyword: "unit",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "integer as seconds",
                source: "from_unix_timestamp!(5000)",
                result: Ok("t'1970-01-01T01:23:20Z'"),
            },
            Example {
                title: "integer as milliseconds",
                source: r#"from_unix_timestamp!(5000, unit: "milliseconds")"#,
                result: Ok("t'1970-01-01T00:00:05Z'"),
            },
            Example {
                title: "integer as nanoseconds",
                source: r#"from_unix_timestamp!(5000, unit: "nanoseconds")"#,
                result: Ok("t'1970-01-01T00:00:05Z'"),
            },
            Example {
                title: "float type invalid",
                source: "from_unix_timestamp!(5000.1)",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:19): unable to coerce float into timestamp"#
                ),
            },
            Example {
                title: "timestamp type invalid",
                source: "from_unix_timestamp(t'2020-01-01T00:00:00Z')",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:19): unable to coerce timestamp into timestamp"#
                ),
            },
            Example {
                title: "boolean type invalid",
                source: "from_unix_timestamp!(true)",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:19): unable to coerce boolean into timestamp"#,
                ),
            },
            Example {
                title: "null",
                source: "from_unix_timestamp!(null)",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:19): unable to coerce null into timestamp"#,
                ),
            },
            Example {
                title: "array",
                source: "from_unix_timestamp!([])",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:17): unable to coerce array into timestamp"#,
                ),
            },
            Example {
                title: "object",
                source: "from_unix_timestamp!({})",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:17): unable to coerce object into timestamp"#,
                ),
            },
            Example {
                title: "regex",
                source: "from_unix_timestamp!(r'foo')",
                result: Err(
                    r#"function call error for "from_unix_timestamp" at (0:21): unable to coerce regex into timestamp"#,
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

        let unit = arguments
            .optional_enum("unit", Unit::all_value().as_slice())?
            .map(|s| {
                Unit::from_str(&s.try_bytes_utf8_lossy().expect("unit not bytes"))
                    .expect("validated enum")
            })
            .unwrap_or_default();

        Ok(FromUnixTimestampFn { value, unit }.as_expr())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    #[default]
    Seconds,
    Milliseconds,
    Nanoseconds,
}

impl Unit {
    fn all_value() -> Vec<Value> {
        use Unit::{Milliseconds, Nanoseconds, Seconds};

        vec![Seconds, Milliseconds, Nanoseconds]
            .into_iter()
            .map(|u| u.as_str().into())
            .collect::<Vec<_>>()
    }

    const fn as_str(self) -> &'static str {
        use Unit::{Milliseconds, Nanoseconds, Seconds};

        match self {
            Seconds => "seconds",
            Milliseconds => "milliseconds",
            Nanoseconds => "nanoseconds",
        }
    }
}

impl FromStr for Unit {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use Unit::{Milliseconds, Nanoseconds, Seconds};

        match s {
            "seconds" => Ok(Seconds),
            "milliseconds" => Ok(Milliseconds),
            "nanoseconds" => Ok(Nanoseconds),
            _ => Err("unit not recognized"),
        }
    }
}

#[derive(Debug, Clone)]
struct FromUnixTimestampFn {
    value: Box<dyn Expression>,
    unit: Unit,
}

impl FunctionExpression for FromUnixTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let unit = self.unit;
        from_unix_timestamp(value, unit)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        self.value
            .type_def(state)
            .fallible_unless(Kind::timestamp())
            .with_kind(Kind::timestamp())
    }
}

#[cfg(test)]
#[allow(overflowing_literals)]
mod tests {
    use super::*;
    use crate::compiler::expression::Literal;
    use crate::compiler::TimeZone;
    use std::collections::BTreeMap;

    #[test]
    fn out_of_range_integer() {
        let mut object: Value = BTreeMap::new().into();
        let mut runtime_state = state::RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
        let f = FromUnixTimestampFn {
            value: Box::new(Literal::Integer(9_999_999_999_999)),
            unit: Unit::default(),
        };
        let string = f.resolve(&mut ctx).err().unwrap().message();
        assert_eq!(string, r#"unable to coerce 9999999999999 into timestamp"#)
    }

    test_function![
        from_unix_timestamp => FromUnixTimestamp;

        integer {
             args: func_args![value: 1_431_648_000],
             want: Ok(chrono::Utc.ymd(2015, 5, 15).and_hms_opt(0, 0, 0).expect("invalid timestamp")),
             tdef: TypeDef::timestamp().fallible(),
        }

        integer_seconds {
            args: func_args![value: 1_609_459_200_i64, unit: "seconds"],
            want: Ok(chrono::Utc.ymd(2021, 1, 1).and_hms_milli(0,0,0,0)),
            tdef: TypeDef::timestamp().fallible(),
        }

        integer_milliseconds {
            args: func_args![value: 1_609_459_200_000_i64, unit: "milliseconds"],
            want: Ok(chrono::Utc.ymd(2021, 1, 1).and_hms_milli(0,0,0,0)),
            tdef: TypeDef::timestamp().fallible(),
        }

        integer_nanoseconds {
            args: func_args![value: 1_609_459_200_000_000_000_i64, unit: "nanoseconds"],
            want: Ok(chrono::Utc.ymd(2021, 1, 1).and_hms_milli(0,0,0,0)),
            tdef: TypeDef::timestamp().fallible(),
        }

        float {
            args: func_args![value: 5.123],
            want: Err("unable to coerce float into timestamp"),
            tdef: TypeDef::timestamp().fallible(),
        }

        float_milliseconds {
            args: func_args![value: 5.123, unit: "milliseconds"],
            want: Err("unable to coerce float into timestamp"),
            tdef: TypeDef::timestamp().fallible(),
        }
    ];
}
