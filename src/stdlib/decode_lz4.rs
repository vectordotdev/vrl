use crate::compiler::prelude::*;
use lz4_flex::block::decompress_size_prepended;
use lz4_flex::frame::FrameDecoder;
use nom::AsBytes;

fn decode_lz4(value: Value) -> Resolved {
    const FRAME_MAGIC_LE: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

    let value = value.try_bytes()?;

    // evaluate if value is lz4 frame encoded by checking the magic number.
    if value.starts_with(&FRAME_MAGIC_LE) {
        let mut buf = Vec::new();
        let mut decoder = FrameDecoder::new(std::io::Cursor::new(value));
        let result = std::io::copy(&mut decoder, &mut buf);
        match result {
            Ok(_) => Ok(Value::Bytes(buf.into())),
            Err(_) => Err("unable to decode value with lz4 decoder".into()),
        }
    } else {
        // value is not lz4 frame encoded, use block decompressor.
        let result = decompress_size_prepended(value.as_bytes());
        match result {
            Ok(buf) => Ok(Value::Bytes(buf.into())),
            Err(_) => Err("unable to decode value with lz4 decoder".into()),
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

        Ok(DecodeLz4Fn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }
}

#[derive(Clone, Debug)]
struct DecodeLz4Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DecodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        decode_lz4(value)
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
            args: func_args![value: value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes())],
            want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
            tdef: TypeDef::bytes().fallible(),
        }

        right_lz4_frame {
            args: func_args![value: value!(decode_base64("BCJNGGBAgiwAAIBUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLgAAAAA=").as_bytes())],
            want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
            tdef: TypeDef::bytes().fallible(),
        }

        wrong_lz4 {
            args: func_args![value: value!("xxxxxxxxx")],
            want: Err("unable to decode value with lz4 decoder"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
