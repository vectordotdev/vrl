use crate::compiler::prelude::*;
use snap::raw::Decoder;

fn decode_snappy(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    let mut decoder = Decoder::new();
    let result = decoder.decompress_vec(&value);

    match result {
        Ok(buf) => Ok(Value::Bytes(buf.into())),
        Err(_) => Err("unable to decode value with Snappy decoder".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DecodeSnappy;

impl Function for DecodeSnappy {
    fn identifier(&self) -> &'static str {
        "decode_snappy"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"decode_snappy!(decode_base64!("LKxUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg=="))"#,
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

        Ok(DecodeSnappyFn { value }.as_expr())
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
struct DecodeSnappyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DecodeSnappyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        decode_snappy(value)
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
        decode_snappy => DecodeSnappy;

        right_snappy {
            args: func_args![value: value!(decode_base64("LKxUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg==").as_bytes())],
            want: Ok(value!(b"The quick brown fox jumps over 13 lazy dogs.")),
            tdef: TypeDef::bytes().fallible(),
        }

        wrong_snappy {
            args: func_args![value: value!("some_bytes")],
            want: Err("unable to decode value with Snappy decoder"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
