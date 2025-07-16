use std::io::Read;

use flate2::read::ZlibEncoder;
use nom::AsBytes;

use crate::compiler::prelude::*;

const MAX_COMPRESSION_LEVEL: u32 = 10;

fn encode_zlib(value: Value, compression_level: Option<Value>) -> Resolved {
    let compression_level = match compression_level {
        None => flate2::Compression::default(),
        Some(value) => {
            // TODO consider removal options
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
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
    ZlibEncoder::new(value.as_bytes(), compression_level)
        .read_to_end(&mut buf)
        .expect("zlib compression failed, please report");

    Ok(Value::Bytes(buf.into()))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeZlib;

impl Function for EncodeZlib {
    fn identifier(&self) -> &'static str {
        "encode_zlib"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"encode_base64(encode_zlib("encode_me"))"#,
            result: Ok("eJxLzUvOT0mNz00FABI5A6A="),
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

        Ok(EncodeZlibFn {
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
struct EncodeZlibFn {
    value: Box<dyn Expression>,
    compression_level: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeZlibFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        let compression_level = self
            .compression_level
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        encode_zlib(value, compression_level)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let is_compression_level_valid_constant = if let Some(level) = &self.compression_level {
            match level.resolve_constant(state) {
                Some(Value::Integer(level)) => level <= i64::from(MAX_COMPRESSION_LEVEL),
                _ => false,
            }
        } else {
            true
        };

        TypeDef::bytes().maybe_fallible(!is_compression_level_valid_constant)
    }
}

#[cfg(test)]
mod test {
    use crate::value;

    use super::*;

    fn encode(text: &str, level: flate2::Compression) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(text.as_bytes(), level);
        let mut output = vec![];
        encoder.read_to_end(&mut output).unwrap();
        output
    }

    test_function![
        encode_zlib => EncodeZlib;

        with_defaults {
            args: func_args![value: value!("you_have_successfully_decoded_me.congratulations.you_are_breathtaking.")],
            want: Ok(value!(encode("you_have_successfully_decoded_me.congratulations.you_are_breathtaking.", flate2::Compression::default()).as_bytes())),
            tdef: TypeDef::bytes().infallible(),
        }

        with_custom_compression_level {
            args: func_args![value: value!("you_have_successfully_decoded_me.congratulations.you_are_breathtaking."), compression_level: 9],
            want: Ok(value!(encode("you_have_successfully_decoded_me.congratulations.you_are_breathtaking.", flate2::Compression::new(9)).as_bytes())),
            tdef: TypeDef::bytes().infallible(),
        }

        invalid_constant_compression {
            args: func_args![value: value!("d"), compression_level: 11],
            want: Err("compression level must be <= 10"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
