use crate::compiler::prelude::*;
use lz4_flex::block::compress_prepend_size;
use nom::AsBytes;

fn encode_lz4(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    let encoded = compress_prepend_size(value.as_bytes());
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
            source: r#"encode_base64(encode_lz4!("The quick brown fox jumps over 13 lazy dogs."))"#,
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

        Ok(EncodeLz4Fn { value }.as_expr())
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
struct EncodeLz4Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeLz4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        encode_lz4(value)
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
    use base64::Engine;
    use nom::AsBytes;

    fn decode_base64(text: &str) -> Vec<u8> {
        let engine = base64::engine::GeneralPurpose::new(
            &base64::alphabet::STANDARD,
            base64::engine::general_purpose::GeneralPurposeConfig::new(),
        );

        engine.decode(text).expect("Cannot decode from Base64")
    }

    test_function![
        encode_lz4 => EncodeLz4;

        success {
            args: func_args![value: value!("The quick brown fox jumps over 13 lazy dogs.")],
            want: Ok(value!(decode_base64("LAAAAPAdVGhlIHF1aWNrIGJyb3duIGZveCBqdW1wcyBvdmVyIDEzIGxhenkgZG9ncy4=").as_bytes())),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
