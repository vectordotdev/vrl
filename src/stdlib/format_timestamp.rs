use crate::compiler::prelude::*;
use crate::compiler::TimeZone;
use chrono::{
    format::{strftime::StrftimeItems, Item},
    DateTime, Utc,
};

fn format_timestamp_with_tz(ts: Value, format: Value, timezone: Option<Value>) -> Resolved {
    let ts: DateTime<Utc> = ts.try_timestamp()?;

    let format_bytes = format.try_bytes()?;
    let format = String::from_utf8_lossy(&format_bytes);

    let timezone_bytes = timezone.map(VrlValueConvert::try_bytes).transpose()?;
    let timezone = timezone_bytes.as_ref().map(|b| String::from_utf8_lossy(b));

    try_format_with_timezone(ts, &format, timezone.as_deref()).map(Into::into)
}

#[derive(Clone, Copy, Debug)]
pub struct FormatTimestamp;

impl Function for FormatTimestamp {
    fn identifier(&self) -> &'static str {
        "format_timestamp"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::TIMESTAMP,
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

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let format = arguments.required("format");
        let timezone = arguments.optional("timezone");

        Ok(FormatTimestampFn {
            value,
            format,
            timezone,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "format timestamp",
                source: r#"format_timestamp!(t'2021-02-10T23:32:00+00:00', format: "%d %B %Y %H:%M")"#,
                result: Ok("10 February 2021 23:32"),
            },
            Example {
                title: "format timestamp with tz",
                source: r#"format_timestamp!(t'2021-02-10T23:32:00+00:00', format: "%d %B %Y %H:%M", timezone: "Europe/Berlin")"#,
                result: Ok("11 February 2021 00:32"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct FormatTimestampFn {
    value: Box<dyn Expression>,
    format: Box<dyn Expression>,
    timezone: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FormatTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.format.resolve(ctx)?;
        let ts = self.value.resolve(ctx)?;
        let tz = self
            .timezone
            .as_ref()
            .map(|tz| tz.resolve(ctx))
            .transpose()?;

        format_timestamp_with_tz(ts, bytes, tz)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

fn try_format_with_timezone(
    dt: DateTime<Utc>,
    format: &str,
    timezone: Option<&str>,
) -> ExpressionResult<String> {
    let items = StrftimeItems::new(format)
        .map(|item| match item {
            Item::Error => Err("invalid format".into()),
            _ => Ok(item),
        })
        .collect::<ExpressionResult<Vec<_>>>()?;

    let timezone = timezone
        .map(|timezone| {
            TimeZone::parse(timezone).ok_or(format!("unable to parse timezone: {timezone}"))
        })
        .transpose()?;

    match timezone {
        Some(TimeZone::Named(tz)) => Ok(dt
            .with_timezone(&tz)
            .format_with_items(items.into_iter())
            .to_string()),
        Some(TimeZone::Local) => Ok(dt
            .with_timezone(&chrono::Local)
            .format_with_items(items.into_iter())
            .to_string()),
        None => Ok(dt.format_with_items(items.into_iter()).to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use chrono::TimeZone;

    test_function![
        format_timestamp => FormatTimestamp;

        invalid {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%Q INVALID"],
            want: Err("invalid format"),
            tdef: TypeDef::bytes().fallible(),
        }

        valid_secs {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%s"],
            want: Ok(value!("10")),
            tdef: TypeDef::bytes().fallible(),
        }

        date {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%+"],
            want: Ok(value!("1970-01-01T00:00:10+00:00")),
            tdef: TypeDef::bytes().fallible(),
        }

        tz {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%+",
                             timezone: "Europe/Berlin"],
            want: Ok(value!("1970-01-01T01:00:10+01:00")),
            tdef: TypeDef::bytes().fallible(),
        }

        tz_local {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%s",
                             timezone: "local"],
            want: Ok(value!("10")), // Check that there is no error for the local timezone
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_tz {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%+",
                             timezone: "llocal"],
            want: Err("unable to parse timezone: llocal"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
