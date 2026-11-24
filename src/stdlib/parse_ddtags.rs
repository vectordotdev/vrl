use crate::compiler::prelude::*;
use std::collections::BTreeMap;

static DEFAULT_MULTIVALUE: Value = Value::Boolean(true);

const PARAMETERS: &[Parameter] = &[
    Parameter::required(
        "value",
        kind::BYTES,
        "The Datadog tag string to parse (comma-separated key:value pairs).",
    ),
    Parameter::optional(
        "multivalue",
        kind::BOOLEAN,
        "If true (default), all values are wrapped in arrays to support duplicate keys. If false, the first value for each key wins.",
    )
    .default(&DEFAULT_MULTIVALUE),
];

/// Yields `(key, value)` pairs, skipping empty segments and empty keys.
/// Splits on the first colon only, so values may contain colons (URLs, ARNs).
fn iter_tags(input: &str) -> impl Iterator<Item = (&str, Value)> {
    input.split(',').filter_map(|segment| {
        let segment = segment.trim();
        let (key, value) = match segment.split_once(':') {
            Some((key, value)) => (key.trim(), Value::from(value.trim())),
            None => (segment, Value::Boolean(true)),
        };
        (!key.is_empty()).then_some((key, value))
    })
}

fn collect_multivalue(input: &str) -> Value {
    let mut map: BTreeMap<KeyString, Vec<Value>> = BTreeMap::new();
    for (key, value) in iter_tags(input) {
        map.entry(KeyString::from(key)).or_default().push(value);
    }
    Value::Object(map.into_iter().map(|(k, v)| (k, Value::Array(v))).collect())
}

fn collect_single_value(input: &str) -> Value {
    let mut map = ObjectMap::new();
    for (key, value) in iter_tags(input) {
        map.entry(KeyString::from(key)).or_insert(value);
    }
    Value::Object(map)
}

fn parse_ddtags(value: &Value, multivalue: Value) -> Resolved {
    let input = value.try_bytes_utf8_lossy()?;
    Ok(if multivalue.try_boolean()? {
        collect_multivalue(&input)
    } else {
        collect_single_value(&input)
    })
}

#[derive(Clone, Copy, Debug)]
pub struct ParseDdtags;

impl Function for ParseDdtags {
    fn identifier(&self) -> &'static str {
        "parse_ddtags"
    }

    fn description(&self) -> &'static str {
        indoc! {r#"
            Parses the `value` as a Datadog tag string: comma-separated `key:value` pairs
            such as `"env:prod,host:server1,host:server2"`.

            When `multivalue` is `true` (the default), every value is wrapped in an array so
            that duplicate keys are preserved. When `multivalue` is `false`, the first
            value for each key wins and values are stored as plain strings.
        "#}
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a string."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "When `multivalue` is `true`, returns an object mapping each key to an array of strings (or booleans for standalone keys).",
            "When `multivalue` is `false`, returns an object mapping each key to a single string (or boolean). Duplicate keys are resolved in favor of the first occurrence.",
        ]
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            Tags without a colon become standalone keys with a boolean `true` value.
            Only the first colon separates the key from the value, so values can contain
            colons. Whitespace around keys and values is trimmed. Empty segments from
            leading, trailing, or consecutive commas are ignored.
        "}]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse Datadog tags (multivalue, default)",
                source: r#"parse_ddtags!("env:prod,host:server1")"#,
                result: Ok(r#"{"env": ["prod"], "host": ["server1"]}"#),
            },
            example! {
                title: "Parse Datadog tags with duplicate keys",
                source: r#"parse_ddtags!("env:prod,host:a,host:b")"#,
                result: Ok(r#"{"env": ["prod"], "host": ["a", "b"]}"#),
            },
            example! {
                title: "Parse Datadog tags (single-value mode)",
                source: r#"parse_ddtags!("env:prod,host:a,host:b", multivalue: false)"#,
                result: Ok(r#"{"env": "prod", "host": "a"}"#),
            },
            example! {
                title: "Standalone keys and colons in values",
                source: r#"parse_ddtags!("novalue,url:https://example.com:8080/path", multivalue: false)"#,
                result: Ok(r#"{"novalue": true, "url": "https://example.com:8080/path"}"#),
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
        let multivalue = arguments.optional("multivalue");

        Ok(ParseDdtagsFn { value, multivalue }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseDdtagsFn {
    value: Box<dyn Expression>,
    multivalue: Option<Box<dyn Expression>>,
}

impl FunctionExpression for ParseDdtagsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let multivalue = self
            .multivalue
            .map_resolve_with_default(ctx, || DEFAULT_MULTIVALUE.clone())?;
        parse_ddtags(&value, multivalue)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        type_def()
    }
}

// `multivalue` may be dynamic, so the type covers both the plain and the array form.
fn type_def() -> TypeDef {
    let scalar = Kind::bytes() | Kind::boolean();
    TypeDef::object(Collection::from_unknown(
        scalar.clone() | Kind::array(Collection::from_unknown(scalar)),
    ))
    .fallible()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        parse_ddtags => ParseDdtags;

        basic_multivalue {
            args: func_args![value: "env:prod,host:server1"],
            want: Ok(value!({"env": ["prod"], "host": ["server1"]})),
            tdef: type_def(),
        }

        duplicate_keys_multivalue {
            args: func_args![value: "env:prod,host:a,host:b"],
            want: Ok(value!({"env": ["prod"], "host": ["a", "b"]})),
            tdef: type_def(),
        }

        duplicate_keys_single_value_first_wins {
            args: func_args![value: "env:prod,host:a,host:b", multivalue: false],
            want: Ok(value!({"env": "prod", "host": "a"})),
            tdef: type_def(),
        }

        standalone_key_multivalue {
            args: func_args![value: "env:prod,standalone"],
            want: Ok(value!({"env": ["prod"], "standalone": [true]})),
            tdef: type_def(),
        }

        standalone_key_single_value {
            args: func_args![value: "env:prod,standalone", multivalue: false],
            want: Ok(value!({"env": "prod", "standalone": true})),
            tdef: type_def(),
        }

        empty_input {
            args: func_args![value: ""],
            want: Ok(value!({})),
            tdef: type_def(),
        }

        whitespace_trimming {
            args: func_args![value: " env : prod , host : server1 ", multivalue: false],
            want: Ok(value!({"env": "prod", "host": "server1"})),
            tdef: type_def(),
        }

        multiple_colons_in_value {
            args: func_args![value: "url:http://example.com:8080/path", multivalue: false],
            want: Ok(value!({"url": "http://example.com:8080/path"})),
            tdef: type_def(),
        }

        trailing_and_leading_commas {
            args: func_args![value: ",env:prod,,host:a,", multivalue: false],
            want: Ok(value!({"env": "prod", "host": "a"})),
            tdef: type_def(),
        }

        empty_key_skipped {
            args: func_args![value: ":value,env:prod", multivalue: false],
            want: Ok(value!({"env": "prod"})),
            tdef: type_def(),
        }

        realistic_ddtags {
            args: func_args![value: "env:prod,host:server1,region:us-east-1,service:web,version:1.2.3,host:server2,team:platform"],
            want: Ok(value!({
                "env": ["prod"],
                "host": ["server1", "server2"],
                "region": ["us-east-1"],
                "service": ["web"],
                "team": ["platform"],
                "version": ["1.2.3"],
            })),
            tdef: type_def(),
        }
    ];
}
