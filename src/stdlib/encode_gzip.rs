use std::io::Read;

use flate2::read::GzEncoder;
use nom::AsBytes;

use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_COMPRESSION_LEVEL: LazyLock<Value> = LazyLock::new(|| Value::Integer(6));

const MAX_COMPRESSION_LEVEL: u32 = 10;

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to encode.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "compression_level",
            kind: kind::INTEGER,
            required: false,
            description: "The default compression level.",
            default: Some(&DEFAULT_COMPRESSION_LEVEL),
            enum_variants: None,
        },
    ]
});

fn encode_gzip(value: Value, compression_level: Value) -> Resolved {
    // TODO consider removal options
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let level = compression_level.try_integer()? as u32;
    let compression_level = if level > MAX_COMPRESSION_LEVEL {
        return Err(format!("compression level must be <= {MAX_COMPRESSION_LEVEL}").into());
    } else {
        flate2::Compression::new(level)
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

    fn usage(&self) -> &'static str {
        "Encodes the `value` to [Gzip](https://www.gzip.org/)."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Encode to Gzip",
            source: r#"encode_base64(encode_gzip("please encode me"))"#,
            result: Ok("H4sIAAAAAAAA/yvISU0sTlVIzUvOT0lVyE0FAI4R4vcQAAAA"),
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
        PARAMETERS.as_slice()
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
            .map_resolve_with_default(ctx, || DEFAULT_COMPRESSION_LEVEL.clone())?;

        encode_gzip(value, compression_level)
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
        let mut encoder = GzEncoder::new(text.as_bytes(), level);
        let mut output = vec![];
        encoder.read_to_end(&mut output).unwrap();
        output
    }

    test_function![
        encode_gzip => EncodeGzip;

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
            args: func_args![value: value!("test"), compression_level: 11],
            want: Err("compression level must be <= 10"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
