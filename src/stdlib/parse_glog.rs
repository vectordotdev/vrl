use crate::compiler::prelude::*;
use chrono::{offset::TimeZone, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;

fn parse_glog(bytes: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let message = String::from_utf8_lossy(&bytes);
    let mut log = ObjectMap::new();
    let captures = REGEX_GLOG
        .captures(&message)
        .ok_or("failed parsing glog message")?;
    if let Some(level) = captures.name("level").map(|capture| capture.as_str()) {
        let level = match level {
            "I" => Ok("info"),
            "W" => Ok("warning"),
            "E" => Ok("error"),
            "F" => Ok("fatal"),
            _ => Err(format!(r#"unrecognized log level "{level}""#)),
        }?;

        log.insert("level".into(), Value::Bytes(level.to_owned().into()));
    }
    if let Some(timestamp) = captures.name("timestamp").map(|capture| capture.as_str()) {
        log.insert(
            "timestamp".into(),
            Value::Timestamp(
                Utc.datetime_from_str(timestamp, "%Y%m%d %H:%M:%S%.f")
                    .map_err(|error| format!(r#"failed parsing timestamp {timestamp}: {error}"#))?,
            ),
        );
    }
    if let Some(id) = captures.name("id").map(|capture| capture.as_str()) {
        log.insert(
            "id".into(),
            Value::Integer(id.parse().map_err(|_| "failed parsing id")?),
        );
    }
    if let Some(file) = captures.name("file").map(|capture| capture.as_str()) {
        log.insert("file".into(), Value::Bytes(file.to_owned().into()));
    }
    if let Some(line) = captures.name("line").map(|capture| capture.as_str()) {
        log.insert(
            "line".into(),
            Value::Integer(line.parse().map_err(|_| "failed parsing line")?),
        );
    }
    if let Some(message) = captures.name("message").map(|capture| capture.as_str()) {
        log.insert("message".into(), Value::Bytes(message.to_owned().into()));
    }
    Ok(log.into())
}

static REGEX_GLOG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)                                                     # Ignore whitespace and comments in the regex expression.
        ^\s*                                                        # Start with any number of whitespaces.
        (?P<level>\w)                                               # Match one word character (expecting `I`,`W`,`E` or `F`).
        (?P<timestamp>\d{4}\d{2}\d{2}\s\d{2}:\d{2}:\d{2}\.\d{6})    # Match YYYYMMDD hh:mm:ss.ffffff.
        \s                                                          # Match one whitespace.
        (?P<id>\d+)                                                 # Match at least one digit.
        \s                                                          # Match one whitespace.
        (?P<file>.+):(?P<line>\d+)                                  # Match any character (greedily), ended by `:` and at least one digit.
        \]\s                                                        # Match `]` and one whitespace.
        (?P<message>.*?)                                            # Match any characters (non-greedily).
        \s*$                                                        # Match any number of whitespaces to be stripped from the end.
    ")
                                                 .expect("failed compiling regex for glog")
});

#[derive(Clone, Copy, Debug)]
pub struct ParseGlog;

impl Function for ParseGlog {
    fn identifier(&self) -> &'static str {
        "parse_glog"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "valid",
            source: r#"parse_glog!("I20210131 14:48:54.411655 15520 main.c++:9] Hello world!")"#,
            result: Ok(indoc! { r#"{
                "file": "main.c++",
                "id": 15520,
                "level": "info",
                "line": 9,
                "message": "Hello world!",
                "timestamp": "2021-01-31T14:48:54.411655Z"
            }"#}),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ParseGlogFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }
}

#[derive(Debug, Clone)]
struct ParseGlogFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseGlogFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.value.resolve(ctx)?;
        parse_glog(bytes)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).fallible()
    }
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("level".into(), Kind::bytes()),
        ("timestamp".into(), Kind::timestamp()),
        ("id".into(), Kind::integer()),
        ("file".into(), Kind::bytes()),
        ("line".into(), Kind::integer()),
        ("message".into(), Kind::bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use crate::btreemap;
    use chrono::DateTime;

    use super::*;

    test_function![
        parse_glog => ParseGlog;

        log_line_valid {
            args: func_args![value: "I20210131 14:48:54.411655 15520 main.c++:9] Hello world!"],
            want: Ok(btreemap! {
                "level" => "info",
                "timestamp" => Value::Timestamp(DateTime::parse_from_rfc3339("2021-01-31T14:48:54.411655Z").unwrap().into()),
                "id" => 15520,
                "file" => "main.c++",
                "line" => 9,
                "message" => "Hello world!",
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        log_line_valid_strip_whitespace {
            args: func_args![value: "\n  I20210131 14:48:54.411655 15520 main.c++:9] Hello world!  \n"],
            want: Ok(btreemap! {
                "level" => "info",
                "timestamp" => Value::Timestamp(DateTime::parse_from_rfc3339("2021-01-31T14:48:54.411655Z").unwrap().into()),
                "id" => 15520,
                "file" => "main.c++",
                "line" => 9,
                "message" => "Hello world!",
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        log_line_invalid {
            args: func_args![value: "not a glog line"],
            want: Err("failed parsing glog message"),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        log_line_invalid_log_level {
            args: func_args![value: "X20210131 14:48:54.411655 15520 main.c++:9] Hello world!"],
            want: Err(r#"unrecognized log level "X""#),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        log_line_invalid_timestamp {
            args: func_args![value: "I20210000 14:48:54.411655 15520 main.c++:9] Hello world!"],
            want: Err("failed parsing timestamp 20210000 14:48:54.411655: input is out of range"),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        log_line_invalid_id {
            args: func_args![value: "I20210131 14:48:54.411655 99999999999999999999999999999 main.c++:9] Hello world!"],
            want: Err("failed parsing id"),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
