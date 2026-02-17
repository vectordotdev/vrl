use crate::compiler::prelude::*;
use lz4_flex::block::{compress, compress_prepend_size};
use nom::AsBytes;
use std::sync::LazyLock;

static DEFAULT_PREPEND_SIZE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));

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
            keyword: "prepend_size",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether to prepend the original size to the compressed data.",
            default: Some(&DEFAULT_PREPEND_SIZE),
            enum_variants: None,
        },
    ]
});

fn encode_lz4(value: Value, prepend_size: bool) -> Resolved {
    let value = value.try_bytes()?;
    if prepend_size {
        let encoded = compress_prepend_size(value.as_bytes());
        return Ok(Value::Bytes(encoded.into()));
    }
    let encoded = compress(value.as_bytes());
    Ok(Value::Bytes(encoded.into()))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeLz4;

impl Function for EncodeLz4 {
    fn identifier(&self) -> &'static str {
        "encode_lz4"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Decodes the `value` (an lz4 string) into its original string. `buf_size` is the size of the buffer to decode into, this must be equal to or larger than the uncompressed size.
            If `prepended_size` is set to `true`, it expects the original uncompressed size to be prepended to the compressed data.
            `prepended_size` is useful for some implementations of lz4 that require the original size to be known before decoding.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Encode to Lz4",
            source: r#"encode_base64(encode_lz4!("The quick brown fox jumps over 13 lazy dogs.", true))"#,
            result: Ok("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4="),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let prepend_size = arguments.optional("prepend_size");

        Ok(EncodeLz4Fn {
            value,
            prepend_size,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }
}

#[derive(Clone, Debug)]
struct EncodeLz4Fn {
    value: Box<dyn Expression>,
    prepend_size: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let prepend_size = self
            .prepend_size
            .map_resolve_with_default(ctx, || DEFAULT_PREPEND_SIZE.clone())?
            .try_boolean()?;

        encode_lz4(value, prepend_size)
    }

    fn type_def(&self, _state: &state::TypeState) -> TypeDef {
        // Always fallible due to the possibility of decoding errors that VRL can't detect
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;
    use nom::AsBytes;

    fn decode_base64(text: &str) -> Vec<u8> {
        base64_simd::STANDARD
            .decode_to_vec(text)
            .expect("Cannot decode from Base64")
    }

    test_function![
        encode_lz4 => EncodeLz4;

        success {
            args: func_args![value: value!("The quick brown fox jumps over 13 lazy dogs."), prepend_size: value!(true)],
            want: Ok(value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes())),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
