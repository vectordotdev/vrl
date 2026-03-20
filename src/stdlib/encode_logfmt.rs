use crate::compiler::prelude::*;
use std::sync::LazyLock;

use super::encode_key_value::{DEFAULT_FIELDS_ORDERING, EncodeKeyValueFn};

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required(
            "value",
            kind::OBJECT,
            "The value to convert to a logfmt string.",
        ),
        Parameter::optional("fields_ordering", kind::ARRAY, "The ordering of fields to preserve. Any fields not in this list are listed unordered, after all ordered fields.")
            .default(&DEFAULT_FIELDS_ORDERING),
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct EncodeLogfmt;

impl Function for EncodeLogfmt {
    fn identifier(&self) -> &'static str {
        "encode_logfmt"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to [logfmt](https://brandur.org/logfmt)."
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
        // The encode_logfmt function is just an alias for `encode_key_value` with the following
        // parameters for the delimiters.
        let key_value_delimiter = Some(expr!("="));
        let field_delimiter = Some(expr!(" "));
        let flatten_boolean = Some(expr!(true));

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
