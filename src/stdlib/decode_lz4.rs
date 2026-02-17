use crate::compiler::prelude::*;
use lz4_flex::block::{decompress, decompress_size_prepended};
use lz4_flex::frame::FrameDecoder;
use std::io;
use std::sync::LazyLock;

static DEFAULT_BUF_SIZE: LazyLock<Value> = LazyLock::new(|| Value::Integer(1_000_000));
static DEFAULT_PREPENDED_SIZE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

const LZ4_FRAME_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter::required("value", kind::BYTES, "The lz4 block data to decode."),
        Parameter::optional("buf_size", kind::INTEGER, "The size of the buffer to decode into, this must be equal to or larger than the uncompressed size.")
            .default(&DEFAULT_BUF_SIZE),
        Parameter::optional("prepended_size", kind::BOOLEAN, "Some implementations of lz4 require the original uncompressed size to be prepended to the compressed data.")
            .default(&DEFAULT_PREPENDED_SIZE),
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct DecodeLz4;

impl Function for DecodeLz4 {
    fn identifier(&self) -> &'static str {
        "decode_lz4"
    }

    fn usage(&self) -> &'static str {
        "Decodes the `value` (an lz4 string) into its original string. `buf_size` is the size of the buffer to decode into, this must be equal to or larger than the uncompressed size.
        If `prepended_size` is set to `true`, it expects the original uncompressed size to be prepended to the compressed data.
        `prepended_size` is useful for some implementations of lz4 that require the original size to be known before decoding."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` unable to decode value with lz4 frame decoder.",
            "`value` unable to decode value with lz4 block decoder.",
            "`value` unable to decode because the output is too large for the buffer.",
            "`value` unable to decode because the prepended size is not a valid integer.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "LZ4 block with prepended size",
                source: r#"decode_lz4!(decode_base64!("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4="), prepended_size: true)"#,
                result: Ok("The quick brown fox jumps over 13 lazy dogs."),
            },
            example! {
                title: "Decode Lz4 data without prepended size.",
                source: r#"decode_lz4!(decode_base64!("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA="))"#,
                result: Ok("The quick brown fox jumps over 13 lazy dogs."),
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
        let buf_size = arguments.optional("buf_size");
        let prepended_size = arguments.optional("prepended_size");

        Ok(DecodeLz4Fn {
            value,
            buf_size,
            prepended_size,
        }
        .as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }
}

#[derive(Clone, Debug)]
struct DecodeLz4Fn {
    value: Box<dyn Expression>,
    buf_size: Option<Box<dyn Expression>>,
    prepended_size: Option<Box<dyn Expression>>,
}

impl FunctionExpression for DecodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let buf_size = self
            .buf_size
            .map_resolve_with_default(ctx, || DEFAULT_BUF_SIZE.clone())?
            .try_integer()?;
        let prepended_size = self
            .prepended_size
            .map_resolve_with_default(ctx, || DEFAULT_PREPENDED_SIZE.clone())?
            .try_boolean()?;

        let buffer_size: usize;
        if let Ok(sz) = u32::try_from(buf_size) {
            buffer_size = sz as usize;
        } else {
            // If the buffer size is too large, we default to a maximum size
            buffer_size = usize::MAX;
        }
        decode_lz4(value, buffer_size, prepended_size)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        // Always fallible due to the possibility of decoding errors that VRL can't detect
        TypeDef::bytes().fallible()
    }
}

fn decode_lz4(value: Value, buf_size: usize, prepended_size: bool) -> Resolved {
    let compressed_data = value.try_bytes()?;

    if is_lz4_frame(&compressed_data) {
        decode_lz4_frame(&compressed_data, buf_size)
    } else {
        decode_lz4_block(&compressed_data, buf_size, prepended_size)
    }
}

fn is_lz4_frame(data: &[u8]) -> bool {
    data.starts_with(&LZ4_FRAME_MAGIC)
}

fn decode_lz4_frame(compressed_data: &[u8], initial_capacity: usize) -> Resolved {
    let mut output_buffer = Vec::with_capacity(initial_capacity);
    let mut decoder = FrameDecoder::new(std::io::Cursor::new(compressed_data));

    match io::copy(&mut decoder, &mut output_buffer) {
        Ok(_) => Ok(Value::Bytes(output_buffer.into())),
        Err(e) => Err(format!("unable to decode value with lz4 frame decoder: {e}").into()),
    }
}

fn decode_lz4_block(compressed_data: &[u8], buf_size: usize, prepended_size: bool) -> Resolved {
    let decompression_result = if prepended_size {
        // The compressed data includes the original size as a prefix
        decompress_size_prepended(compressed_data)
    } else {
        // We need to provide the buffer size for decompression
        decompress(compressed_data, buf_size)
    };

    match decompression_result {
        Ok(decompressed_data) => Ok(Value::Bytes(decompressed_data.into())),
        Err(e) => Err(format!("unable to decode value with lz4 block decoder: {e}").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    use nom::AsBytes;

    // Define a constant for 256 KB, used in tests
    const KB_256: usize = 262_144;

    fn decode_base64(text: &str) -> Vec<u8> {
        base64_simd::STANDARD
            .decode_to_vec(text)
            .expect("Cannot decode from Base64")
    }

    test_function![
    decode_lz4 => DecodeLz4;

    right_lz4_block {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDk5IGxhenkgZG9ncy4=").as_bytes()), prepended_size: value!(true)],
        want: Ok(value!(b"The quick brown fox jumps over 99 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_frame {
        args: func_args![value: value!(decode_base64("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA=").as_bytes())],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_block_no_prepend_size_with_buffer_size {
        args: func_args![value: value!(decode_base64("8B1UaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg==").as_bytes()), buf_size: value!(KB_256), prepended_size: value!(false)],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_frame_grow_buffer_size_from_zero {
        args: func_args![value: value!(decode_base64("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA=").as_bytes()), buf_size: value!(0), prepended_size: value!(false)],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4_block_grow_buffer_size_from_zero_no_prepended_size {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes()), buf_size: value!(0), prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 block decoder: provided output is too small for the decompressed data, actual 0, expected 2"),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4 {
        args: func_args![value: value!("xxxxxxxxx"), buf_size: value!(10), prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 block decoder: expected another byte, found none"),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4_block_false_prepended_size {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes()), prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 block decoder: the offset to copy is not contained in the decompressed buffer"),
        tdef: TypeDef::bytes().fallible(),
    }];
}
