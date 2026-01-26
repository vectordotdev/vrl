use crate::compiler::prelude::*;
use std::str::FromStr;
use std::sync::LazyLock;

static DEFAULT_UNIT: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("seconds")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::TIMESTAMP,
            required: true,
            description: "The timestamp to convert into a Unix timestamp.",
            default: None,
        },
        Parameter {
            keyword: "unit",
            kind: kind::BYTES,
            required: false,
            description: "The time unit.",
            default: Some(&DEFAULT_UNIT),
        },
    ]
});

fn to_unix_timestamp(value: Value, unit: Unit) -> Resolved {
    let ts = value.try_timestamp()?;
    let time = match unit {
        Unit::Seconds => ts.timestamp(),
        Unit::Milliseconds => ts.timestamp_millis(),
        Unit::Microseconds => ts.timestamp_micros(),
        Unit::Nanoseconds => match ts.timestamp_nanos_opt() {
            None => return Err(ValueError::OutOfRange(Kind::timestamp()).into()),
            Some(nanos) => nanos,
        },
    };
    Ok(time.into())
}

#[derive(Clone, Copy, Debug)]
pub struct ToUnixTimestamp;

impl Function for ToUnixTimestamp {
    fn identifier(&self) -> &'static str {
        "to_unix_timestamp"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Converts the `value` timestamp into a [Unix timestamp](https://en.wikipedia.org/wiki/Unix_time).

            Returns the number of seconds since the Unix epoch by default. To return the number in milliseconds or nanoseconds, set the `unit` argument to `milliseconds` or `nanoseconds`.
        "}
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Convert to a Unix timestamp (seconds)",
                source: "to_unix_timestamp(t'2021-01-01T00:00:00+00:00')",
                result: Ok("1609459200"),
            },
            example! {
                title: "Convert to a Unix timestamp (milliseconds)",
                source: r#"to_unix_timestamp(t'2021-01-01T00:00:00Z', unit: "milliseconds")"#,
                result: Ok("1609459200000"),
            },
            example! {
                title: "Convert to a Unix timestamp (microseconds)",
                source: r#"to_unix_timestamp(t'2021-01-01T00:00:00Z', unit: "microseconds")"#,
                result: Ok("1609459200000000"),
            },
            example! {
                title: "Convert to a Unix timestamp (nanoseconds)",
                source: r#"to_unix_timestamp(t'2021-01-01T00:00:00Z', unit: "nanoseconds")"#,
                result: Ok("1609459200000000000"),
            },
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        let unit = Unit::from_str(
            &arguments
                .optional_enum("unit", Unit::all_value().as_slice(), state)?
                .unwrap_or_else(|| DEFAULT_UNIT.clone())
                .try_bytes_utf8_lossy()
                .expect("unit not bytes"),
        )
        .expect("validated enum");

        Ok(ToUnixTimestampFn { value, unit }.as_expr())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    #[default]
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

impl Unit {
    fn all_value() -> Vec<Value> {
        use Unit::{Microseconds, Milliseconds, Nanoseconds, Seconds};

        vec![Seconds, Milliseconds, Microseconds, Nanoseconds]
            .into_iter()
            .map(|u| u.as_str().into())
            .collect::<Vec<_>>()
    }

    const fn as_str(self) -> &'static str {
        use Unit::{Microseconds, Milliseconds, Nanoseconds, Seconds};

        match self {
            Seconds => "seconds",
            Milliseconds => "milliseconds",
            Microseconds => "microseconds",
            Nanoseconds => "nanoseconds",
        }
    }
}

impl FromStr for Unit {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        use Unit::{Microseconds, Milliseconds, Nanoseconds, Seconds};

        match s {
            "seconds" => Ok(Seconds),
            "milliseconds" => Ok(Milliseconds),
            "microseconds" => Ok(Microseconds),
            "nanoseconds" => Ok(Nanoseconds),
            _ => Err("unit not recognized"),
        }
    }
}

#[derive(Debug, Clone)]
struct ToUnixTimestampFn {
    value: Box<dyn Expression>,
    unit: Unit,
}

impl FunctionExpression for ToUnixTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let unit = self.unit;

        to_unix_timestamp(value, unit)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().infallible()
    }
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};

    use super::*;

    test_function![
        to_unix_timestamp => ToUnixTimestamp;

        seconds {
            args: func_args![value: Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(),
                             unit: "seconds"
            ],
            want: Ok(1_609_459_200_i64),
            tdef: TypeDef::integer().infallible(),
        }

        milliseconds {
            args: func_args![value: Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(),
                             unit: "milliseconds"
            ],
            want: Ok(1_609_459_200_000_i64),
            tdef: TypeDef::integer().infallible(),
        }

        microseconds {
            args: func_args![value: Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(),
                             unit: "microseconds"
            ],
            want: Ok(1_609_459_200_000_000_i64),
            tdef: TypeDef::integer().infallible(),
        }

        nanoseconds {
             args: func_args![value: Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap(),
                              unit: "nanoseconds"
             ],
             want: Ok(1_609_459_200_000_000_000_i64),
             tdef: TypeDef::integer().infallible(),
         }
         out_of_range {
             args: func_args![value: Utc.with_ymd_and_hms(0, 1, 1, 0, 0, 0).unwrap(),
                              unit: "nanoseconds"
             ],
             want: Err("can't convert out of range timestamp"),
             tdef: TypeDef::integer().infallible(),
         }
    ];
}
