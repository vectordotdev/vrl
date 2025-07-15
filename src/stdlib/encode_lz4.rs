use crate::compiler::prelude::*;
use lz4_flex::block::{compress, compress_prepend_size};
use nom::AsBytes;

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

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
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
        let prepend_size = arguments
            .optional("prepend_size")
            .unwrap_or_else(|| expr!(true));

        Ok(EncodeLz4Fn {
            value,
            prepend_size,
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
                keyword: "prepend_size",
                kind: kind::BOOLEAN,
                required: false,
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodeLz4Fn {
    value: Box<dyn Expression>,
    prepend_size: Box<dyn Expression>,
}

impl FunctionExpression for EncodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let prepend_size = self.prepend_size.resolve(ctx)?.try_boolean()?;

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
