use crate::compiler::prelude::*;

fn encode_json(value: Value, pretty: bool) -> Value {
    // With `vrl::Value` it should not be possible to get `Err`.

    let result = if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    };

    match result {
        Ok(value) => value.into(),
        Err(error) => unreachable!("unable encode to json: {}", error),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeJson;

impl Function for EncodeJson {
    fn identifier(&self) -> &'static str {
        "encode_json"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::ANY,
                required: true,
            },
            Parameter {
                keyword: "pretty",
                kind: kind::BOOLEAN,
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
        let pretty = arguments.optional("pretty");

        Ok(EncodeJsonFn { value, pretty }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "encode object",
                source: r#"encode_json({"field": "value", "another": [1,2,3]})"#,
                result: Ok(r#"s'{"another":[1,2,3],"field":"value"}'"#),
            },
            Example {
                title: "encode object as a pretty-printed JSON",
                source: r#"encode_json({"field": "value", "another": [1,2,3]}, true)"#,
                result: Ok(
                    r#""{\n  \"another\": [\n    1,\n    2,\n    3\n  ],\n  \"field\": \"value\"\n}""#,
                ),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodeJsonFn {
    value: Box<dyn Expression>,
    pretty: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeJsonFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let pretty = match &self.pretty {
            Some(pretty) => pretty.resolve(ctx)?.try_boolean()?,
            None => false,
        };
        Ok(encode_json(value, pretty))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{DateTime, Utc};
    use regex::Regex;

    test_function![
        encode_json => EncodeJson;

        bytes {
            args: func_args![value: "hello"],
            want: Ok(r#""hello""#),
            tdef: TypeDef::bytes().infallible(),
        }

        bytes_pretty {
            args: func_args![value: "hello", pretty: true],
            want: Ok(r#""hello""#),
            tdef: TypeDef::bytes().infallible(),
        }

        integer {
            args: func_args![value: 42],
            want: Ok("42"),
            tdef: TypeDef::bytes().infallible(),
        }

        integer_pretty {
            args: func_args![value: 42, pretty: true],
            want: Ok("42"),
            tdef: TypeDef::bytes().infallible(),
        }

        float {
            args: func_args![value: 42f64],
            want: Ok("42.0"),
            tdef: TypeDef::bytes().infallible(),
        }

        float_pretty {
            args: func_args![value: 42f64, pretty: true],
            want: Ok("42.0"),
            tdef: TypeDef::bytes().infallible(),
        }

        boolean {
            args: func_args![value: false],
            want: Ok("false"),
            tdef: TypeDef::bytes().infallible(),
        }

        boolean_pretty {
            args: func_args![value: false, pretty: true],
            want: Ok("false"),
            tdef: TypeDef::bytes().infallible(),
        }

        map {
            args: func_args![value: Value::from_iter([(String::from("field"), Value::from("value"))])],
            want: Ok(r#"{"field":"value"}"#),
            tdef: TypeDef::bytes().infallible(),
        }

        map_pretty {
            args: func_args![value: Value::from_iter([(String::from("field"), Value::from("value"))]), pretty: true],
            want: Ok("{\n  \"field\": \"value\"\n}"),
            tdef: TypeDef::bytes().infallible(),
        }

        array {
            args: func_args![value: vec![1, 2, 3]],
            want: Ok("[1,2,3]"),
            tdef: TypeDef::bytes().infallible(),
        }

        array_pretty {
            args: func_args![value: vec![1, 2, 3], pretty: true],
            want: Ok("[\n  1,\n  2,\n  3\n]"),
            tdef: TypeDef::bytes().infallible(),
        }

        timestamp {
            args: func_args![
                value: DateTime::parse_from_str("1983 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
                    .unwrap()
                    .with_timezone(&Utc)
            ],
            want: Ok(r#""1983-04-13T12:09:14.274Z""#),
            tdef: TypeDef::bytes().infallible(),
        }

        timestamp_pretty {
            args: func_args![
                value: DateTime::parse_from_str("1983 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
                    .unwrap()
                    .with_timezone(&Utc),
                pretty: true
            ],
            want: Ok(r#""1983-04-13T12:09:14.274Z""#),
            tdef: TypeDef::bytes().infallible(),
        }

        regex {
            args: func_args![value: Regex::new("^a\\d+$").unwrap()],
            want: Ok(r#""^a\\d+$""#),
            tdef: TypeDef::bytes().infallible(),
        }

        regex_pretty {
            args: func_args![value: Regex::new("^a\\d+$").unwrap(), pretty: true],
            want: Ok(r#""^a\\d+$""#),
            tdef: TypeDef::bytes().infallible(),
        }

        null {
            args: func_args![value: Value::Null],
            want: Ok("null"),
            tdef: TypeDef::bytes().infallible(),
        }

        null_pretty {
            args: func_args![value: Value::Null, pretty: true],
            want: Ok("null"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
