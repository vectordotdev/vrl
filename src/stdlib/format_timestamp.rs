use crate::compiler::prelude::*;
use chrono::{
    format::{strftime::StrftimeItems, Item},
    DateTime, Local, TimeZone, Utc,
};
use chrono_tz::Tz;

fn format_timestamp_with_tz<Tz2: TimeZone>(bytes: Value, ts: Value, tz: &Tz2) -> Resolved
where
    Tz2::Offset: fmt::Display,
{
    let bytes = bytes.try_bytes()?;
    let format = String::from_utf8_lossy(&bytes);
    let ts = ts.try_timestamp()?;
    let ts = ts.with_timezone(tz);

    try_format_tz(&ts, &format).map(Into::into)
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
                keyword: "tz",
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
        let tz: Option<Box<dyn Expression>> = arguments.optional("tz");

        Ok(FormatTimestampFn { value, format, tz }.as_expr())
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
                source: r#"format_timestamp!(t'2021-02-10T23:32:00+00:00', format: "%d %B %Y %H:%M", tz: "Europe/Berlin")"#,
                result: Ok("11 February 2021 00:32"),
            },
        ]
    }
}

#[derive(Debug, Clone)]
struct FormatTimestampFn {
    value: Box<dyn Expression>,
    format: Box<dyn Expression>,
    tz: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FormatTimestampFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.format.resolve(ctx)?;
        let ts = self.value.resolve(ctx)?;

        match self.tz.clone() {
            None => format_timestamp_with_tz(bytes, ts, &Utc),
            Some(tz) => {
                let tz = &tz.resolve(ctx)?.try_bytes()?;
                let tz = String::from_utf8_lossy(tz);
                match tz {
                    std::borrow::Cow::Borrowed("Local") => {
                        format_timestamp_with_tz(bytes, ts, &Local)
                    }
                    _ => {
                        let tz: Tz = tz.parse().unwrap();
                        format_timestamp_with_tz(bytes, ts, &tz)
                    }
                }
            }
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

fn try_format_tz<Tz2: TimeZone>(dt: &DateTime<Tz2>, format: &str) -> ExpressionResult<String>
where
    Tz2::Offset: fmt::Display,
{
    let items = StrftimeItems::new(format)
        .map(|item| match item {
            Item::Error => Err("invalid format".into()),
            _ => Ok(item),
        })
        .collect::<ExpressionResult<Vec<_>>>()?;

    Ok(dt.format_with_items(items.into_iter()).to_string())
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
                             tz: "Europe/Berlin"],
            want: Ok(value!("1970-01-01T01:00:10+01:00")),
            tdef: TypeDef::bytes().fallible(),
        }

        tz_local {
            args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"),
                             format: "%s",
                             tz: "Local"],
            want: Ok(value!("10")), // Check that there is no error for the Local timezone
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
