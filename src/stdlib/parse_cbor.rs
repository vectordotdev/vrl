use crate::compiler::prelude::*;
use crate::stdlib::json_utils::json_type_def::json_type_def;
use ciborium::de::from_reader;
use zstd::zstd_safe::WriteBuf;

fn parse_cbor(value: Value) -> Resolved {
    let bytes = value.try_bytes()?;
    let value = from_reader(bytes.as_slice()).map_err(|e| format!("unable to parse cbor: {e}"))?;
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
pub struct ParseCbor;

impl Function for ParseCbor {
    fn identifier(&self) -> &'static str {
        "parse_cbor"
    }

    fn summary(&self) -> &'static str {
        "parse a string to a CBOR type"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Parses the provided `value` as CBOR.
        "}
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "object",
                source: r#"parse_cbor!(decode_base64!("oWVmaWVsZGV2YWx1ZQ=="))"#,
                result: Ok(r#"{ "field": "value" }"#),
            },
            Example {
                title: "array",
                source: r#"parse_cbor!(decode_base64!("gvUA"))"#,
                result: Ok("[true, 0]"),
            },
            Example {
                title: "string",
                source: r#"parse_cbor!(decode_base64!("ZWhlbGxv"))"#,
                result: Ok("hello"),
            },
            Example {
                title: "integer",
                source: r#"parse_cbor!(decode_base64!("GCo="))"#,
                result: Ok("42"),
            },
            Example {
                title: "float",
                source: r#"parse_cbor!(decode_base64!("+0BFEKPXCj1x"))"#,
                result: Ok("42.13"),
            },
            Example {
                title: "boolean",
                source: r#"parse_cbor!(decode_base64!("9A=="))"#,
                result: Ok("false"),
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
        Ok(ParseCborFn { value }.as_expr())
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
struct ParseCborFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ParseCborFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        parse_cbor(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        json_type_def()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    use nom::AsBytes;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("tests/data/cbor")
    }

    fn read_cbor_file(cbor_bin_message_path: &str) -> Vec<u8> {
        fs::read(test_data_dir().join(cbor_bin_message_path)).unwrap()
    }

    test_function![
        parse_cbor => ParseCbor;

        parses {
            args: func_args![ value: value!(read_cbor_file("simple.cbor").as_bytes()) ],
            want: Ok(value!({ field: "value" })),
            tdef: json_type_def(),
        }

        complex_cbor {
            args: func_args![ value: value!(read_cbor_file("complex.cbor").as_bytes()) ],
            want: Ok(value!({ object: {string: "value", number: 42, array: ["hello", "world"], boolean: false} })),
            tdef: json_type_def(),
        }
    ];
}
