use crate::compiler::prelude::*;
use lz4_flex::block::{decompress, decompress_size_prepended, DecompressError};
use lz4_flex::frame::FrameDecoder;
use nom::AsBytes;

fn decode_lz4(value: Value, buf_size: usize, prepended_size: bool) -> Resolved {
    const FRAME_MAGIC_LE: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

    let value = value.try_bytes()?;

    // evaluate if value is lz4 frame encoded by checking the magic number.
    if value.starts_with(&FRAME_MAGIC_LE) {
        let mut buf = Vec::with_capacity(buf_size);
        // FrameDecoder doesn't require or have a value for prepending the uncompressed size to the compressed data.
        let mut decoder = FrameDecoder::new(std::io::Cursor::new(value));
        let result = std::io::copy(&mut decoder, &mut buf);
        match result {
            Ok(_) => Ok(Value::Bytes(buf.into())),
            Err(_) => Err("unable to decode value with lz4 decoder".into()),
        }
    } else {
        // value is not lz4 frame encoded, use block decompressor.
        let result: Result<Vec<u8>, DecompressError>;
        // some lz4 block compressors prepend the size of the original data to the compressed data.
        // this is often to improve performance when decompressing as the size of the buffer can be known in advance.
        // if prepended_size is true, we will use decompress_size_prepended, otherwise
        // we will use decompress which requires a buffer size.
        if prepended_size {
            result = decompress_size_prepended(value.as_bytes());
        } else {
            result = decompress(value.as_bytes(), buf_size);
        }
        match result {
            Ok(buf) => Ok(Value::Bytes(buf.into())),
            Err(e) => {
                let msg = format!("unable to decode value with lz4 decoder {e}");
                Err(msg.into())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DecodeLz4;

impl Function for DecodeLz4 {
    fn identifier(&self) -> &'static str {
        "decode_lz4"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"decode_lz4!(decode_base64!("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4="))"#,
            result: Ok("The quick brown fox jumps over 13 lazy dogs."),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let buf_size = arguments.optional("buf_size").unwrap_or_else(|| expr!(0));
        let prepended_size = arguments
            .optional("use_prepended_size")
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
                kind: kind::INTEGER,
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

        let buffer_size = buf_size as usize;

        decode_lz4(value, buffer_size, prepended_size)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        // Always fallible due to the possibility of decoding errors that VRL can't detect
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    use nom::AsBytes;

    fn decode_base64(text: &str) -> Vec<u8> {
        base64_simd::STANDARD
            .decode_to_vec(text)
            .expect("Cannot decode from Base64")
    }

    test_function![
    decode_lz4 => DecodeLz4;

    right_lz4_block {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes()), use_prepended_size: value!(true)],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_frame {
        args: func_args![value: value!(decode_base64("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA=").as_bytes())],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_block_no_prepend_size_with_buffer_size {
        args: func_args![value: value!(decode_base64("8B1UaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg==").as_bytes()), buf_size: value!(262144), use_prepended_size: value!(false)],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    right_lz4_frame_grow_buffer_size_from_zero {
        args: func_args![value: value!(decode_base64("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA=").as_bytes()), buf_size: value!(0), use_prepended_size: value!(false)],
        want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4_block_grow_buffer_size_from_zero_no_prepended_size {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes()), buf_size: value!(0), use_prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 decoder provided output is too small for the decompressed data, actual 0, expected 2"),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4 {
        args: func_args![value: value!("xxxxxxxxx"), buf_size: value!(10), use_prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 decoder expected another byte, found none"),
        tdef: TypeDef::bytes().fallible(),
    }

    wrong_lz4_block_false_prepended_size {
        args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes()), use_prepended_size: value!(false)],
        want: Err("unable to decode value with lz4 decoder provided output is too small for the decompressed data, actual 0, expected 2"),
        tdef: TypeDef::bytes().fallible(),
    }];
}
