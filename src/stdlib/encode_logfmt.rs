use crate::compiler::prelude::*;

use super::encode_key_value::EncodeKeyValueFn;

#[derive(Clone, Copy, Debug)]
pub struct EncodeLogfmt;

impl Function for EncodeLogfmt {
    fn identifier(&self) -> &'static str {
        "encode_logfmt"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to [logfmt](https://brandur.org/logfmt)."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT,
                required: true,
                description: "The value to convert to a logfmt string.",
            },
            Parameter {
                keyword: "fields_ordering",
                kind: kind::ARRAY,
                required: false,
                description: "The ordering of fields to preserve. Any fields not in this list are listed unordered, after all ordered fields.",
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        // The encode_logfmt function is just an alias for `encode_key_value` with the following
        // parameters for the delimiters.
        let key_value_delimiter = expr!("=");
        let field_delimiter = expr!(" ");
        let flatten_boolean = expr!(true);

        let value = arguments.required("value");
        let fields = arguments.optional("fields_ordering");

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
                title: "Encode to logfmt (no ordering)",
                source: r#"encode_logfmt({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info"})"#,
                result: Ok(r#"lvl=info msg="This is a message" ts=2021-06-05T17:20:00Z"#),
            },
            example! {
                title: "Encode to logfmt (fields ordering)",
                source: r#"encode_logfmt!({"ts": "2021-06-05T17:20:00Z", "msg": "This is a message", "lvl": "info", "log_id": 12345}, ["ts", "lvl", "msg"])"#,
                result: Ok(r#"ts=2021-06-05T17:20:00Z lvl=info msg="This is a message" log_id=12345"#),
            },
            example! {
                title: "Encode to logfmt (nested fields)",
                source: r#"encode_logfmt({"agent": {"name": "foo"}, "log": {"file": {"path": "my.log"}}, "event": "log"})"#,
                result: Ok(r"agent.name=foo event=log log.file.path=my.log"),
            },
            example! {
                title: "Encode to logfmt (nested fields ordering)",
                source: r#"encode_logfmt!({"agent": {"name": "foo"}, "log": {"file": {"path": "my.log"}}, "event": "log"}, ["event", "log.file.path", "agent.name"])"#,
                result: Ok(r"event=log log.file.path=my.log agent.name=foo"),
            },
        ]
    }
}
