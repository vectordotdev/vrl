use crate::compiler::prelude::*;
use crate::core::encode_key_value;
use crate::value::KeyString;
use std::sync::LazyLock;

/// Also used by `encode_logfmt`.
pub(crate) fn encode_key_value(
    fields: Option<Value>,
    value: Value,
    key_value_delimiter: &Value,
    field_delimiter: &Value,
    flatten_boolean: Value,
) -> ExpressionResult<Value> {
    let fields = match fields {
        None => Ok(vec![]),
        Some(fields) => resolve_fields(fields),
    }?;
    let object = value.try_object()?;
    let key_value_delimiter = key_value_delimiter.try_bytes_utf8_lossy()?;
    let field_delimiter = field_delimiter.try_bytes_utf8_lossy()?;
    let flatten_boolean = flatten_boolean.try_boolean()?;
    Ok(encode_key_value::to_string(
        &object,
        &fields[..],
        &key_value_delimiter,
        &field_delimiter,
        flatten_boolean,
    )
    .expect("Should always succeed.")
    .into())
}

pub(super) static DEFAULT_FIELDS_ORDERING: LazyLock<Value> = LazyLock::new(|| Value::Array(vec![]));
static DEFAULT_KEY_VALUE_DELIMITER: LazyLock<Value> =
    LazyLock::new(|| Value::Bytes(Bytes::from("=")));
static DEFAULT_FIELD_DELIMITER: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from(" ")));
static DEFAULT_FLATTEN_BOOLEAN: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::OBJECT,
            required: true,
            description: "The value to convert to a string.",
            default: None,
        },
        Parameter {
            keyword: "fields_ordering",
            kind: kind::ARRAY,
            required: false,
            description: "The ordering of fields to preserve. Any fields not in this list are listed unordered, after all ordered fields.",
            default: Some(&DEFAULT_FIELDS_ORDERING),
        },
        Parameter {
            keyword: "key_value_delimiter",
            kind: kind::BYTES,
            required: false,
            description: "The string that separates the key from the value.",
            default: Some(&DEFAULT_KEY_VALUE_DELIMITER),
        },
        Parameter {
            keyword: "field_delimiter",
            kind: kind::BYTES,
            required: false,
            description: "The string that separates each key-value pair.",
            default: Some(&DEFAULT_FIELD_DELIMITER),
        },
        Parameter {
            keyword: "flatten_boolean",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether to encode key-value with a boolean value as a standalone key if `true` and nothing if `false`.",
            default: Some(&DEFAULT_FLATTEN_BOOLEAN),
        },
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct EncodeKeyValue;

impl Function for EncodeKeyValue {
    fn identifier(&self) -> &'static str {
        "encode_key_value"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` into key-value format with customizable delimiters. Default delimiters match the [logfmt](https://brandur.org/logfmt) format."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`fields_ordering` contains a non-string element."]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn notices(&self) -> &'static [&'static str] {
        &["If `fields_ordering` is specified then the function is fallible else it is infallible."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let fields = arguments.optional("fields_ordering");

        let key_value_delimiter = arguments.optional("key_value_delimiter");
        let field_delimiter = arguments.optional("field_delimiter");
        let flatten_boolean = arguments.optional("flatten_boolean");

        Ok(EncodeKeyValueFn {
            value,
            fields,
            key_value_delimiter,
            field_delimiter,
            flatten_boolean,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Encode with default delimiters (no ordering)",
                source: r#"encode_key_value({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info"})"#,
                result: Ok(r#"lvl=info msg="This is a message" ts=2021-06-05T17:20:00Z"#),
            },
            example! {
                title: "Encode with default delimiters (fields ordering)",
                source: r#"encode_key_value!({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info", "log_id": 12345}, ["ts", "lvl", "msg"])"#,
                result: Ok(r#"ts=2021-06-05T17:20:00Z lvl=info msg="This is a message" log_id=12345"#),
            },
            example! {
                title: "Encode with default delimiters (nested fields)",
                source: r#"encode_key_value({"agent": {"name": "foo"}, "log": {"file": {"path": "my.log"}}, "event": "log"})"#,
                result: Ok(r"agent.name=foo event=log log.file.path=my.log"),
            },
            example! {
                title: "Encode with default delimiters (nested fields ordering)",
                source: r#"encode_key_value!({"agent": {"name": "foo"}, "log": {"file": {"path": "my.log"}}, "event": "log"}, ["event", "log.file.path", "agent.name"])"#,
                result: Ok(r"event=log log.file.path=my.log agent.name=foo"),
            },
            example! {
                title: "Encode with custom delimiters (no ordering)",
                source: r#"encode_key_value({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info"}, field_delimiter: ",", key_value_delimiter: ":")"#,
                result: Ok(r#"lvl:info,msg:"This is a message",ts:2021-06-05T17:20:00Z"#),
            },
            example! {
                title: "Encode with custom delimiters and flatten boolean",
                source: r#"encode_key_value({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info", "beta": true, "dropped": false}, field_delimiter: ",", key_value_delimiter: ":", flatten_boolean: true)"#,
                result: Ok(r#"beta,lvl:info,msg:"This is a message",ts:2021-06-05T17:20:00Z"#),
            },
        ]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EncodeKeyValueFn {
    pub(crate) value: Box<dyn Expression>,
    pub(crate) fields: Option<Box<dyn Expression>>,
    pub(crate) key_value_delimiter: Option<Box<dyn Expression>>,
    pub(crate) field_delimiter: Option<Box<dyn Expression>>,
    pub(crate) flatten_boolean: Option<Box<dyn Expression>>,
}

fn resolve_fields(fields: Value) -> ExpressionResult<Vec<KeyString>> {
    let arr = fields.try_array()?;
    arr.iter()
        .enumerate()
        .map(|(idx, v)| {
            v.try_bytes_utf8_lossy()
                .map(|v| v.to_string().into())
                .map_err(|e| format!("invalid field value type at index {idx}: {e}").into())
        })
        .collect()
}

impl FunctionExpression for EncodeKeyValueFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let fields = self
            .fields
            .map_resolve_with_default(ctx, || DEFAULT_FIELDS_ORDERING.clone())?;
        let key_value_delimiter = self
            .key_value_delimiter
            .map_resolve_with_default(ctx, || DEFAULT_KEY_VALUE_DELIMITER.clone())?;
        let field_delimiter = self
            .field_delimiter
            .map_resolve_with_default(ctx, || DEFAULT_FIELD_DELIMITER.clone())?;
        let flatten_boolean = self
            .flatten_boolean
            .map_resolve_with_default(ctx, || DEFAULT_FLATTEN_BOOLEAN.clone())?;

        encode_key_value(
            Some(fields),
            value,
            &key_value_delimiter,
            &field_delimiter,
            flatten_boolean,
        )
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().maybe_fallible(self.fields.is_some())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        btreemap,
        stdlib::parse_key_value::{Whitespace, parse_key_value},
        value,
    };

    use super::*;

    #[test]
    fn test_encode_decode_cycle() {
        let before: Value = {
            let mut map = Value::from(BTreeMap::default());
            map.insert("key", r#"this has a " quote"#);
            map
        };

        let after = parse_key_value(
            &encode_key_value(None, before.clone(), &"=".into(), &" ".into(), true.into())
                .expect("valid key value before"),
            &Value::from("="),
            &Value::from(" "),
            true.into(),
            Whitespace::Lenient,
        )
        .expect("valid key value after");

        assert_eq!(before, after);
    }

    #[test]
    fn test_decode_encode_cycle() {
        let before: Value = r#"key="this has a \" quote""#.into();

        let after = encode_key_value(
            Some(Value::Array(vec![
                "key".into(),
                "has".into(),
                "a".into(),
                r#"""#.into(),
                "quote".into(),
            ])),
            parse_key_value(
                &before,
                &Value::from("="),
                &Value::from(" "),
                true.into(),
                Whitespace::Lenient,
            )
            .expect("valid key value before"),
            &Value::from("="),
            &Value::from(" "),
            true.into(),
        )
        .expect("valid key value after");

        assert_eq!(before, after);
    }

    test_function![
        encode_key_value  => EncodeKeyValue;

        single_element {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info"
                }
            ],
            want: Ok("lvl=info"),
            tdef: TypeDef::bytes().infallible(),
        }

        multiple_elements {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "log_id" => 12345
                }
            ],
            want: Ok("log_id=12345 lvl=info"),
            tdef: TypeDef::bytes().infallible(),
        }

        string_with_spaces {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "msg" => "This is a log message"
                }],
            want: Ok(r#"lvl=info msg="This is a log message""#),
            tdef: TypeDef::bytes().infallible(),
        }

        string_with_quotes {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "msg" => "{\"key\":\"value\"}"
                }],
            want: Ok(r#"lvl=info msg="{\"key\":\"value\"}""#),
            tdef: TypeDef::bytes().infallible(),
        }

        flatten_boolean {
            args: func_args![value:
                btreemap! {
                    "beta" => true,
                    "prod" => false,
                    "lvl" => "info",
                    "msg" => "This is a log message",
                },
                flatten_boolean: value!(true)
            ],
            want: Ok(r#"beta lvl=info msg="This is a log message""#),
            tdef: TypeDef::bytes().infallible(),
        }

        dont_flatten_boolean {
            args: func_args![value:
                btreemap! {
                    "beta" => true,
                    "prod" => false,
                    "lvl" => "info",
                    "msg" => "This is a log message",
                },
                flatten_boolean: value!(false)
            ],
            want: Ok(r#"beta=true lvl=info msg="This is a log message" prod=false"#),
            tdef: TypeDef::bytes().infallible(),
        }

        flatten_boolean_with_custom_delimiters {
            args: func_args![value:
                btreemap! {
                    "tag_a" => "val_a",
                    "tag_b" => "val_b",
                    "tag_c" => true,
                },
                key_value_delimiter: value!(":"),
                field_delimiter: value!(","),
                flatten_boolean: value!(true)
            ],
            want: Ok("tag_a:val_a,tag_b:val_b,tag_c"),
            tdef: TypeDef::bytes().infallible(),
        }
        string_with_characters_to_escape {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "msg" => r#"payload: {"code": 200}\n"#,
                    "another_field" => "some\nfield\\and things",
                    "space key" => "foo"
                }],
            want: Ok(r#"another_field="some\\nfield\\and things" lvl=info msg="payload: {\"code\": 200}\\n" "space key"=foo"#),
            tdef: TypeDef::bytes().infallible(),
        }

        nested_fields {
            args: func_args![value:
                btreemap! {
                    "log" => btreemap! {
                        "file" => btreemap! {
                            "path" => "encode_key_value.rs"
                        },
                    },
                    "agent" => btreemap! {
                        "name" => "vector",
                        "id" => 1234
                    },
                    "network" => btreemap! {
                        "ip" => value!([127, 0, 0, 1]),
                        "proto" => "tcp"
                    },
                    "event" => "log"
                }],
                want: Ok("agent.id=1234 agent.name=vector event=log log.file.path=encode_key_value.rs network.ip.0=127 network.ip.1=0 network.ip.2=0 network.ip.3=1 network.proto=tcp"),
                tdef: TypeDef::bytes().infallible(),
        }

        fields_ordering {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "msg" => "This is a log message",
                    "log_id" => 12345,
                },
                fields_ordering: value!(["lvl", "msg"])
            ],
            want: Ok(r#"lvl=info msg="This is a log message" log_id=12345"#),
            tdef: TypeDef::bytes().fallible(),
        }

        nested_fields_ordering {
            args: func_args![value:
                btreemap! {
                    "log" => btreemap! {
                        "file" => btreemap! {
                            "path" => "encode_key_value.rs"
                        },
                    },
                    "agent" => btreemap! {
                        "name" => "vector",
                    },
                    "event" => "log"
                },
                fields_ordering:  value!(["event", "log.file.path", "agent.name"])
            ],
            want: Ok("event=log log.file.path=encode_key_value.rs agent.name=vector"),
            tdef: TypeDef::bytes().fallible(),
        }

        fields_ordering_invalid_field_type {
            args: func_args![value:
                btreemap! {
                    "lvl" => "info",
                    "msg" => "This is a log message",
                    "log_id" => 12345,
                },
                fields_ordering: value!(["lvl", 2])
            ],
            want: Err(format!(r"invalid field value type at index 1: {}",
                    ValueError::Expected {
                        got: Kind::integer(),
                        expected: Kind::bytes()
                    })),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
