use crate::compiler::prelude::*;
use lz4_flex::block::{decompress, decompress_size_prepended};
use lz4_flex::frame::FrameDecoder;
use std::io;

const LZ4_FRAME_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];
const LZ4_DEFAULT_BUFFER_SIZE: usize = 1_000_000; // Default buffer size for decompression 1MB

#[derive(Clone, Copy, Debug)]
pub struct DecodeLz4;

impl Function for DecodeLz4 {
    fn identifier(&self) -> &'static str {
        "decode_lz4"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "LZ4 block with prepended size",
                source: r#"decode_lz4!(decode_base64!("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4="), prepended_size: true)"#,
                result: Ok("The quick brown fox jumps over 13 lazy dogs."),
            },
            Example {
                title: "LZ4 frame format",
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
        let buf_size = arguments
            .optional("buf_size")
            .unwrap_or_else(|| expr!(LZ4_DEFAULT_BUFFER_SIZE));
        let prepended_size = arguments
            .optional("prepended_size")
            .unwrap_or_else(|| expr!(false));

        Ok(DecodeLz4Fn {
            value,
            buf_size,
            prepended_size,
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
                keyword: "buf_size",
                kind: kind::INTEGER,
                required: false,
            },
            Parameter {
                keyword: "prepended_size",
                kind: kind::BOOLEAN,
                required: false,
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct DecodeLz4Fn {
    value: Box<dyn Expression>,
    buf_size: Box<dyn Expression>,
    prepended_size: Box<dyn Expression>,
}

impl FunctionExpression for DecodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let buf_size = self.buf_size.resolve(ctx)?.try_integer()?;
        let prepended_size = self.prepended_size.resolve(ctx)?.try_boolean()?;

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
