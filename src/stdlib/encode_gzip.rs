use std::io::Read;

use flate2::read::GzEncoder;
use nom::AsBytes;

use crate::compiler::prelude::*;

const MAX_COMPRESSION_LEVEL: u32 = 10;

fn encode_gzip(value: Value, compression_level: Option<Value>) -> Resolved {
    let compression_level = match compression_level {
        None => flate2::Compression::default(),
        Some(value) => {
            let level = value.try_integer()? as u32;
            if level > MAX_COMPRESSION_LEVEL {
                return Err(format!("compression level must be <= {MAX_COMPRESSION_LEVEL}").into());
            }
            flate2::Compression::new(level)
        }
    };

    let value = value.try_bytes()?;
    let mut buf = Vec::new();
    // We can safely ignore the error here because the value being read from, `Bytes`, never fails a `read()` call and the value being written to, a `Vec`, never fails a `write()` call
    GzEncoder::new(value.as_bytes(), compression_level)
        .read_to_end(&mut buf)
        .expect("gzip compression failed, please report");

    Ok(Value::Bytes(buf.into()))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeGzip;

impl Function for EncodeGzip {
    fn identifier(&self) -> &'static str {
        "encode_gzip"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"encode_base64(encode_gzip("encode_me"))"#,
            result: Ok("H4sIAAAAAAAA/0vNS85PSY3PTQUAN7ZBnAkAAAA="),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let compression_level = arguments.optional("compression_level");

        Ok(EncodeGzipFn {
            value,
            compression_level,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "compression_level",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodeGzipFn {
    value: Box<dyn Expression>,
    compression_level: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeGzipFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        let compression_level = self
            .compression_level
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        encode_gzip(value, compression_level)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let is_compression_level_valid_constant = if let Some(level) = &self.compression_level {
            if let Some(Value::Integer(level)) = level.resolve_constant(state) {
                level <= i64::from(MAX_COMPRESSION_LEVEL)
            } else {
                false
            }
        } else {
            true
        };

        TypeDef::bytes().maybe_fallible(!is_compression_level_valid_constant)
    }
}

#[cfg(test)]
mod test {
    use base64::Engine;

    use crate::value;

    use super::*;

    fn decode_base64(text: &str) -> Vec<u8> {
        let engine = base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::general_purpose::GeneralPurposeConfig::new(),
        );

        engine.decode(text).expect("Cannot decode from Base64")
    }

    test_function![
        encode_gzip => EncodeGzip;

        with_defaults {
            args: func_args![value: value!("you_have_successfully_decoded_me.congratulations.you_are_breathtaking.")],
            want: Ok(value!(decode_base64("H4sIAAAAAAAA/w3LgQ3AIAgEwI1ciVD8qqmVRKAJ29cBLjWo8weyEIHZHXMmVYhWVHpRRFfb7DHZhy4reQBv0LXB3p2fsVr5AXeBkepGAAAA").as_bytes())),
            tdef: TypeDef::bytes().infallible(),
        }

        with_custom_compression_level {
            args: func_args![value: value!("you_have_successfully_decoded_me.congratulations.you_are_breathtaking."), compression_level: 9],
            want: Ok(value!(decode_base64("H4sIAAAAAAAC/w3LgQ3AIAgEwI1ciVD8qqmVRKAJ29cBLjWo8weyEIHZHXMmVYhWVHpRRFfb7DHZhy4reQBv0LXB3p2fsVr5AXeBkepGAAAA").as_bytes())),
            tdef: TypeDef::bytes().infallible(),
        }
        invalid_constant_compression {
            args: func_args![value: value!("test"), compression_level: 11],
            want: Err("compression level must be <= 10"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
