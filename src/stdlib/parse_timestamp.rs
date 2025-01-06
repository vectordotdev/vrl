use crate::compiler::conversion::Conversion;
use crate::compiler::prelude::*;
use crate::compiler::TimeZone;

fn parse_timestamp(
    value: Value,
    format: Value,
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

            Conversion::parse(format!("timestamp|{format}"), timezone)
                .map_err(|e| e.to_string())?
                .convert(v)
                .map_err(|e| e.to_string().into())
        }
        Value::Timestamp(_) => Ok(value),
        _ => Err("unable to convert value to timestamp".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseTimestamp;

impl Function for ParseTimestamp {
    fn identifier(&self) -> &'static str {
        "parse_timestamp"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid",
                source: r#"parse_timestamp!("11-Feb-2021 16:00 +00:00", format: "%v %R %z")"#,
                result: Ok("t'2021-02-11T16:00:00Z'"),
            },
            Example {
                title: "valid with timezone",
                source: r#"parse_timestamp!("16/10/2019 12:00:00", format: "%d/%m/%Y %H:%M:%S", timezone: "Europe/Paris")"#,
                result: Ok("t'2019-10-16T10:00:00Z'"),
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
            },
            Parameter {
                keyword: "format",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "timezone",
                kind: kind::BYTES,
                required: false,
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
        parse_timestamp(value, format, tz, ctx)
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
                format:"%d/%m/%Y:%H:%M:%S"
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
                format:"%d/%m/%Y:%H:%M:%S",
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
                format:"%d/%m/%Y:%H:%M:%S",
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
                format:"%d/%m/%Y:%H:%M:%S",
                timezone: "Europe/Pariss"
            ],
            want: Err("unable to parse timezone: Europe/Pariss"),
            tdef: TypeDef::timestamp().fallible(),
            tz: TimeZone::default(),
        }
    ];
}
