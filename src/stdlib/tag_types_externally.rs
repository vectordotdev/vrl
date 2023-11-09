use crate::compiler::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct TagTypesExternally;

impl Function for TagTypesExternally {
    fn identifier(&self) -> &'static str {
        "tag_types_externally"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "scalar",
                source: "tag_types_externally(123)",
                result: Ok(r#"{ "integer": 123 }"#),
            },
            Example {
                title: "object",
                source: indoc! {r#"
                    tag_types_externally({
                        "message": "Hello world",
                        "request": {
                            "duration_ms": 67.9
                        }
                    })
                "#},
                result: Ok(
                    r#"{ "message": { "string": "Hello world" }, "request": { "duration_ms": { "float": 67.9 } } }"#,
                ),
            },
            Example {
                title: "array",
                source: r#"tag_types_externally(["foo", "bar"])"#,
                result: Ok(r#"[{ "string": "foo" }, { "string": "bar" }]"#),
            },
            Example {
                title: "null",
                source: r#"tag_types_externally(null)"#,
                result: Ok("null"),
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

        Ok(TagTypesExternallyFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }
}

#[derive(Debug, Clone)]
struct TagTypesExternallyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for TagTypesExternallyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let tagged_externally = tag_type_externally(value);

        Ok(tagged_externally)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        match self.value.type_def(state) {
            td if td.is_array() => TypeDef::array(Collection::any()),
            td if td.is_null() => TypeDef::null(),
            _ => TypeDef::object(Collection::any()),
        }
    }
}

fn tag_type_externally(value: Value) -> Value {
    let (key, value) = match value {
        value @ Value::Bytes(_) => (Some("string"), value),
        value @ Value::Integer(_) => (Some("integer"), value),
        value @ Value::Float(_) => (Some("float"), value),
        value @ Value::Boolean(_) => (Some("boolean"), value),
        Value::Object(object) => (
            None,
            object
                .into_iter()
                .map(|(key, value)| (key, tag_type_externally(value)))
                .collect::<ObjectMap>()
                .into(),
        ),
        Value::Array(array) => (
            None,
            array
                .into_iter()
                .map(tag_type_externally)
                .collect::<Vec<_>>()
                .into(),
        ),
        value @ Value::Timestamp(_) => (Some("timestamp"), value),
        value @ Value::Regex(_) => (Some("regex"), value),
        Value::Null => (None, Value::Null),
    };

    if let Some(key) = key {
        BTreeMap::from([(key.to_owned().into(), value)]).into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use crate::btreemap;
    use chrono::{TimeZone, Utc};
    use regex::Regex;

    use super::*;

    test_function![
        tag_types_externally => TagTypesExternally;

        tag_bytes {
            args: func_args![value: "foo"],
            want: Ok(btreemap! {
                "string" => "foo",
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_integer {
            args: func_args![value: 123],
            want: Ok(btreemap! {
                "integer" => 123
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_float {
            args: func_args![value: 123.45],
            want: Ok(btreemap! {
                "float" => 123.45
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_boolean {
            args: func_args![value: true],
            want: Ok(btreemap! {
                "boolean" => true
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_map {
            args: func_args![value: btreemap! {"foo" => "bar"}],
            want: Ok(btreemap! {
                "foo" => btreemap! {
                    "string" => "bar"
                }
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_array {
            args: func_args![value: vec!["foo"]],
            want: Ok(vec![
                btreemap! {
                    "string" => "foo"
                },
            ]),
            tdef: TypeDef::array(Collection::any()),
        }

        tag_timestamp {
            args: func_args![value: Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)],
            want: Ok(btreemap! {
                "timestamp" => Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_regex {
            args: func_args![value: Regex::new(".*").unwrap()],
            want: Ok(btreemap! {
                "regex" => Regex::new(".*").unwrap()
            }),
            tdef: TypeDef::object(Collection::any()),
        }

        tag_null {
            args: func_args![value: Value::Null],
            want: Ok(Value::Null),
            tdef: TypeDef::null(),
        }
    ];
}
