use crate::compiler::TimeZone;
use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;

fn parse_timestamp(
    value: Value,
    format: &Value,
    timezone: Option<Value>,
    ctx: &Context,
) -> Resolved {
    match value {
        Value::Bytes(v) => {
            let format = format.try_bytes_utf8_lossy()?;

            let timezone_bytes = timezone.map(VrlValueConvert::try_bytes).transpose()?;
            let timezone = timezone_bytes.as_ref().map(|b| String::from_utf8_lossy(b));
            let timezone = timezone
                .as_deref()
                .map(|timezone| {
                    TimeZone::parse(timezone).ok_or(format!("unable to parse timezone: {timezone}"))
                })
                .transpose()?
                .unwrap_or(*ctx.timezone());

            Conversion::timestamp(&format, timezone)
                .convert(v)
                .map_err(|e| e.to_string().into())
        }
        Value::Timestamp(_) => Ok(value),
        _ => Err(format!("unable to convert {} value to timestamp", value.kind_str()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseTimestamp;

impl Function for ParseTimestamp {
    fn identifier(&self) -> &'static str {
        "parse_timestamp"
    }

    fn usage(&self) -> &'static str {
        "Parses the `value` in [strptime](https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers) `format`."
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse timestamp",
                source: r#"parse_timestamp!("10-Oct-2020 16:00+00:00", format: "%v %R %:z")"#,
                result: Ok("t'2020-10-10T16:00:00Z'"),
            },
            example! {
                title: "Parse timestamp with timezone",
                source: r#"parse_timestamp!("16/10/2019 12:00:00", format: "%d/%m/%Y %H:%M:%S", timezone: "Asia/Taipei")"#,
                result: Ok("t'2019-10-16T04:00:00Z'"),
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
        let format = arguments.required("format");
        let timezone = arguments.optional("timezone");

        Ok(ParseTimestampFn {
            value,
            format,
            timezone,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES | kind::TIMESTAMP,
                required: true,
                description: "The text of the timestamp.",
                default: None,
            },
            Parameter {
                keyword: "format",
                kind: kind::BYTES,
                required: true,
                description: "The [strptime](https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers) format.",
                default: None,
            },
            Parameter {
                keyword: "timezone",
                kind: kind::BYTES,
                required: false,
                description: "The [TZ database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones) format. By default, this function parses the timestamp by global [`timezone` option](/docs/reference/configuration//global-options#timezone).
This argument overwrites the setting and is useful for parsing timestamps without a specified timezone, such as `16/10/2019 12:00:00`.",
                default: None,
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct ParseTimestampFn {
    value: Box<dyn Expression>,
    format: Box<dyn Expression>,
    timezone: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let format = self.format.resolve(ctx)?;
        let tz = self
            .timezone
            .as_ref()
            .map(|tz| tz.resolve(ctx))
            .transpose()?;
        parse_timestamp(value, &format, tz, ctx)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::timestamp().fallible(/* always fallible because the format and the timezone need to be parsed at runtime */)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use chrono::{DateTime, Utc};

    test_function![
        parse_timestamp => ParseTimestamp;

        parse_timestamp {
            args: func_args![
                value: DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 12:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc),
                format:"%d/%m/%Y:%H:%M:%S %z"
            ],
            want: Ok(value!(
                DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 12:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc)
            )),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }

        parse_text {
            args: func_args![
                value: "16/10/2019:12:00:00 +0000",
                format: "%d/%m/%Y:%H:%M:%S %z"
            ],
            want: Ok(value!(
                DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 12:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc)
            )),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }

        parse_text_with_tz {
            args: func_args![
                value: "16/10/2019:12:00:00",
                format: "%d/%m/%Y:%H:%M:%S"
            ],
            want: Ok(value!(
                DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 10:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc)
            )),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::Named(chrono_tz::Europe::Paris),
        }

        // test without Daylight Saving Time (DST)
        parse_text_with_timezone_args_no_dst {
            args: func_args![
                value: "31/12/2019:12:00:00",
                format: "%d/%m/%Y:%H:%M:%S",
                timezone: "Europe/Paris"
            ],
            want: Ok(value!(
                DateTime::parse_from_rfc2822("Tue, 31 Dec 2019 11:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc)
            )),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }

        parse_text_with_favor_timezone_args_than_tz_no_dst {
            args: func_args![
                value: "31/12/2019:12:00:00",
                format: "%d/%m/%Y:%H:%M:%S",
                timezone: "Europe/Paris"
            ],
            want: Ok(value!(
                DateTime::parse_from_rfc2822("Tue, 31 Dec 2019 11:00:00 +0000")
                    .unwrap()
                    .with_timezone(&Utc)
            )),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::Named(chrono_tz::Europe::London),
        }

        err_timezone_args {
            args: func_args![
                value: "16/10/2019:12:00:00",
                format: "%d/%m/%Y:%H:%M:%S",
                timezone: "Europe/Pariss"
            ],
            want: Err("unable to parse timezone: Europe/Pariss"),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }

        err_value_null {
            args: func_args![
                value: value!(null),
                format: "%d/%m/%Y:%H:%M:%S",
            ],
            want: Err("unable to convert null value to timestamp"),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }
    ];
}
