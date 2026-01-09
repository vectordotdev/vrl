//use serde_yaml::{
//    //Error,
//    //value::{Value as YamlValue},
//};

use crate::compiler::prelude::*;
use crate::stdlib::json_utils::bom::StripBomFromUTF8;
use crate::stdlib::json_utils::json_type_def::json_type_def;

fn parse_yaml(value: Value) -> Resolved {
    Ok(serde_yaml::from_slice(value.try_bytes()?.strip_bom())
        .map_err(|e| format!("unable to parse yaml: {e}"))?)
}

#[derive(Clone, Copy, Debug)]
pub struct ParseYaml;

impl Function for ParseYaml {
    fn identifier(&self) -> &'static str {
        "parse_yaml"
    }

    fn summary(&self) -> &'static str {
        "parse a string to a YAML type"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Parses the provided `value` as YAML.

            Only YAML types are returned. If you need to convert a `string` into a `timestamp`,
            consider the `parse_timestamp` function.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Parse YAML",
                source: r"parse_yaml!(s'key: val')",
                result: Ok(r#"{ "key": "val" }"#),
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
                    json_array: ["hello", "world"]
                    boolean: false
                    array:
                    - hello
                    - world
            "# ],
            want: Ok(value!({ object: {string: "value", number: 42, json_array: ["hello", "world"], boolean: false, array: ["hello", "world"]} })),
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
                r"unable to parse yaml: found unexpected end of stream at line 1 column 17, while scanning a quoted scalar at line 1 column 11"
            ),
            tdef: json_type_def(),
        }
    ];
}
