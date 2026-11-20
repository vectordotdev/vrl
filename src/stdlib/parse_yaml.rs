use crate::compiler::prelude::*;
use crate::stdlib::json_utils::bom::StripBomFromUTF8;
use crate::stdlib::json_utils::json_type_def::json_type_def;

fn parse_yaml(value: Value) -> Resolved {
    // Preserve the previous parser's behavior: `.inf`/`-.inf` deserialize to
    // infinite floats (only NaN is rejected, by `Value::visit_f64`).
    // Match the behavior of the previous `serde_yaml_ng`-based parser
    // (serde_yaml 0.9 semantics) to keep existing VRL programs compatible:
    // only `true`/`false` are booleans, numeric-looking spellings like `007`
    // or `1_000` stay strings, duplicate keys keep the last value, merge keys
    // are not expanded, and `!!binary` scalars are not base64-decoded.
    let options = serde_saphyr::options! {
        strict_booleans: true,
        leading_zero_numbers_as_strings: true,
        underscored_numbers_as_strings: true,
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::PassThrough,
        merge_keys: serde_saphyr::MergeKeyPolicy::AsOrdinary,
        ignore_binary_tag_for_string: true,
        non_finite_float_policy: serde_saphyr::NonFiniteFloatPolicy::PassThrough,
    };
    Ok(
        serde_saphyr::from_slice_with_options(value.try_bytes()?.strip_bom(), options)
            .map_err(|e| format!("unable to parse yaml: {e}"))?,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct ParseYaml;

impl Function for ParseYaml {
    fn identifier(&self) -> &'static str {
        "parse_yaml"
    }

    fn description(&self) -> &'static str {
        indoc! {"
            Parses the provided `value` as YAML.

            Only YAML types are returned. If you need to convert a `string` into a `timestamp`,
            consider the `parse_timestamp` function.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a valid YAML-formatted payload."]
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
            | kind::INTEGER
            | kind::FLOAT
            | kind::BYTES
            | kind::OBJECT
            | kind::ARRAY
            | kind::NULL
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            Only YAML types are returned. If you need to convert a `string` into a `timestamp`,
            consider the [`parse_timestamp`](#parse_timestamp) function.
        "}]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::BYTES,
            "The string representation of the YAML to parse.",
        )];

        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse simple YAML",
                source: r"parse_yaml!(s'key: val')",
                result: Ok(r#"{ "key": "val" }"#),
            },
            example! {
                title: "Parse YAML",
                source: r#"parse_yaml!(s'
                    object:
                        string: value
                        number: 42
                        boolean: false
                        array:
                        - hello
                        - world
                        json_array: ["hello", "world"]
                ')"#,
                result: Ok(r#"{
                    "object": {
                        "string": "value",
                        "number": 42,
                        "boolean": false,
                        "array": [
                            "hello",
                            "world"
                        ],
                        "json_array": [
                            "hello",
                            "world"
                        ]
                    }
                }"#),
            },
            example! {
                title: "Parse YAML string",
                source: r"parse_yaml!(s'hello')",
                result: Ok("hello"),
            },
            example! {
                title: "Parse YAML quoted string",
                source: r#"parse_yaml!(s'"hello"')"#,
                result: Ok("hello"),
            },
            example! {
                title: "Parse YAML integer",
                source: r#"parse_yaml!("42")"#,
                result: Ok("42"),
            },
            example! {
                title: "Parse YAML float",
                source: r#"parse_yaml!("42.13")"#,
                result: Ok("42.13"),
            },
            example! {
                title: "Parse YAML boolean",
                source: r#"parse_yaml!("false")"#,
                result: Ok("false"),
            },
            example! {
                title: "Parse embedded JSON",
                source: r#"parse_yaml!(s'{"key": "val"}')"#,
                result: Ok(r#"{ "key": "val" }"#),
            },
            example! {
                title: "Parse embedded JSON array",
                source: r#"parse_yaml!("[true, 0]")"#,
                result: Ok("[true, 0]"),
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

        Ok(ParseYamlFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ParseYamlFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseYamlFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_yaml(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        json_type_def()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use ordered_float::NotNan;

    test_function![
        parse_yaml => ParseYaml;

        parses {
            args: func_args![ value: r"
                field: value
            " ],
            want: Ok(value!({ field: "value" })),
            tdef: json_type_def(),
        }

        complex_yaml {
            args: func_args![ value: r#"
                object:
                    string: value
                    number: 42
                    boolean: false
                    array:
                    - hello
                    - world
                    json_array: ["hello", "world"]
            "# ],
            want: Ok(value!({ object: {string: "value", number: 42, boolean: false, array: ["hello", "world"], json_array: ["hello", "world"]} })),
            tdef: json_type_def(),
        }

        parses_json {
            args: func_args![ value: r#"{"field": "value"}"# ],
            want: Ok(value!({ field: "value" })),
            tdef: json_type_def(),
        }

        complex_json {
            args: func_args![ value: r#"{"object": {"string":"value","number":42,"array":["hello","world"],"boolean":false}}"# ],
            want: Ok(value!({ object: {string: "value", number: 42, array: ["hello", "world"], boolean: false} })),
            tdef: json_type_def(),
        }

        incomplete_json_errors {
            args: func_args![ value: r#"{"field": "value"# ],
            want: Err(
                r#"unable to parse yaml: error: line 1 column 11: unclosed quote
 --> <input>:1:11
  |
1 | {"field": "value
  |           ^ unclosed quote"#
            ),
            tdef: json_type_def(),
        }

        parses_inf {
            args: func_args![ value: r"value: .inf" ],
            want: Ok({
                let mut map = ObjectMap::new();
                map.insert("value".into(), Value::Float(NotNan::new(f64::INFINITY).expect("infinity is not NaN")));
                Value::Object(map)
            }),
            tdef: json_type_def(),
        }

        parses_neg_inf {
            args: func_args![ value: "-.inf" ],
            want: Ok(Value::Float(NotNan::new(f64::NEG_INFINITY).expect("infinity is not NaN"))),
            tdef: json_type_def(),
        }

        nan_still_errors {
            args: func_args![ value: "value: .nan" ],
            want: Err(
                r#"unable to parse yaml: error: line 1 column 1: invalid value: floating point `NaN`, expected any valid JSON value
 --> <input>:1:1
  |
1 | value: .nan
  | ^ invalid value: floating point `NaN`, expected any valid JSON value"#
            ),
            tdef: json_type_def(),
        }

        yaml11_booleans_stay_strings {
            args: func_args![ value: "flag: off" ],
            want: Ok(value!({ flag: "off" })),
            tdef: json_type_def(),
        }

        yes_stays_string {
            args: func_args![ value: "flag: yes" ],
            want: Ok(value!({ flag: "yes" })),
            tdef: json_type_def(),
        }

        leading_zero_number_stays_string {
            args: func_args![ value: "n: 007" ],
            want: Ok(value!({ n: "007" })),
            tdef: json_type_def(),
        }

        underscored_number_stays_string {
            args: func_args![ value: "n: 1_000" ],
            want: Ok(value!({ n: "1_000" })),
            tdef: json_type_def(),
        }

        hex_and_octal_still_parse {
            args: func_args![ value: "a: 0x10\nb: 0o17" ],
            want: Ok(value!({ a: 16, b: 15 })),
            tdef: json_type_def(),
        }

        duplicate_keys_last_wins {
            args: func_args![ value: "a: 1\na: 2" ],
            want: Ok(value!({ a: 2 })),
            tdef: json_type_def(),
        }

        merge_keys_not_expanded {
            args: func_args![ value: "base: &b\n  x: 1\nfoo:\n  <<: *b\n  y: 2" ],
            want: Ok(value!({ base: { x: 1 }, foo: { "<<": { x: 1 }, y: 2 } })),
            tdef: json_type_def(),
        }

        binary_tag_not_decoded {
            args: func_args![ value: "bin: !!binary aGk=" ],
            want: Ok(value!({ bin: "aGk=" })),
            tdef: json_type_def(),
        }
    ];
}
