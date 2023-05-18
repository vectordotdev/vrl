use crate::compiler::prelude::*;
use crate::compiler::TimeZone;
use chrono::{
    format::{strftime::StrftimeItems, Item},
    DateTime, Local, Utc,
};
use std::borrow::Borrow;

fn format_timestamp_with_tz(ts: Value, format: Value, timezone: Option<Value>) -> Resolved {
    let format = format.try_bytes()?;
    let format = String::from_utf8_lossy(&format);
    let items = StrftimeItems::new(&format)
        .map(|item| match item {
            Item::Error => Err("invalid format".into()),
            _ => Ok(item),
        })
        .collect::<ExpressionResult<Vec<_>>>()?;

    let ts: DateTime<Utc> = ts.try_timestamp()?;

    let ts = match timezone {
        None => ts.format_with_items(items.into_iter()).to_string(),
        Some(timezone) => try_format_with_timezone(ts, items, timezone)?,
    };

    Ok(ts).map(Into::into)
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
        let timezone: Option<Box<dyn Expression>> = arguments.optional("timezone");

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

        match self.timezone.clone() {
            Some(tz) => {
                let tz = tz.resolve(ctx)?;
                format_timestamp_with_tz(ts, bytes, Some(tz))
            }
            None => format_timestamp_with_tz(ts, bytes, None),
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

fn try_format_with_timezone(
    ts: DateTime<Utc>,
    items: Vec<Item>,
    timezone: Value,
) -> ExpressionResult<String> {
    let timezone = timezone.try_bytes()?;
    let timezone = String::from_utf8_lossy(&timezone);
    let parsed_timezone = TimeZone::parse(timezone.borrow());

    match parsed_timezone {
        Some(parsed_timezone) => match parsed_timezone {
            TimeZone::Local => Ok(ts
                .with_timezone(&Local)
                .format_with_items(items.into_iter())
                .to_string()),
            TimeZone::Named(tz) => Ok(ts
                .with_timezone(&tz)
                .format_with_items(items.into_iter())
                .to_string()),
        },
        None => Err(format!("unable to parse timezone: {timezone}").into()),
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
