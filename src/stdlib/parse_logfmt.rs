use super::parse_key_value::{ParseKeyValueFn, Whitespace};
use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct ParseLogFmt;

impl Function for ParseLogFmt {
    fn identifier(&self) -> &'static str {
        "parse_logfmt"
    }

    fn usage(&self) -> &'static str {
        indoc! {r#"
            Parses the `value` in [logfmt](https://brandur.org/logfmt).

            * Keys and values can be wrapped using the `"` character.
            * `"` characters can be escaped by the `\` character.
            * As per this [logfmt specification](https://pkg.go.dev/github.com/kr/logfmt#section-documentation), the `parse_logfmt` function accepts standalone keys and assigns them a Boolean value of `true`.
        "#}
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a properly formatted key-value string"]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to parse.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse simple logfmt log",
                source: r#"parse_logfmt!("zork=zook zonk=nork")"#,
                result: Ok(r#"{"zork": "zook", "zonk": "nork"}"#),
            },
            example! {
                title: "Parse logfmt log",
                source: indoc! {r#"
                    parse_logfmt!(
                        "@timestamp=\"Sun Jan 10 16:47:39 EST 2021\" level=info msg=\"Stopping all fetchers\" tag#production=stopping_fetchers id=ConsumerFetcherManager-1382721708341 module=kafka.consumer.ConsumerFetcherManager"
                    )
                "#},
                result: Ok(indoc! {r#"{
                    "@timestamp": "Sun Jan 10 16:47:39 EST 2021",
                    "level": "info",
                    "msg": "Stopping all fetchers",
                    "tag#production": "stopping_fetchers",
                    "id": "ConsumerFetcherManager-1382721708341",
                    "module": "kafka.consumer.ConsumerFetcherManager"
                }"#}),
            },
            example! {
                title: "Parse logfmt log with standalone key",
                source: r#"parse_logfmt!("zork=zook plonk zonk=nork")"#,
                result: Ok(r#"{"plonk": true, "zork": "zook", "zonk": "nork"}"#),
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

        // The parse_logfmt function is just an alias for `parse_key_value` with the following
        // parameters for the delimiters.
        let key_value_delimiter = Some(expr!("="));
        let field_delimiter = Some(expr!(" "));
        let whitespace = Whitespace::Lenient;
        let standalone_key = Some(expr!(true));

        Ok(ParseKeyValueFn {
            value,
            key_value_delimiter,
            field_delimiter,
            whitespace,
            standalone_key,
        }
        .as_expr())
    }
}
