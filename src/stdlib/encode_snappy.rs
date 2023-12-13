use crate::compiler::prelude::*;
use snap::raw::Encoder;

fn encode_snappy(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    let mut encoder = Encoder::new();
    let result = encoder.compress_vec(&value);

    match result {
        Ok(buf) => Ok(Value::Bytes(buf.into())),
        Err(_) => Err("unable to encode value with Snappy encoder".into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeSnappy;

impl Function for EncodeSnappy {
    fn identifier(&self) -> &'static str {
        "encode_snappy"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"encode_base64(encode_snappy!("The quick brown fox jumps over 13 lazy dogs."))"#,
            result: Ok("LKxUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg=="),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(EncodeSnappyFn { value }.as_expr())
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
struct EncodeSnappyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeSnappyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        encode_snappy(value)
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
        encode_snappy => EncodeSnappy;

        success {
            args: func_args![value: value!("The quick brown fox jumps over 13 lazy dogs.")],
            want: Ok(value!(decode_base64("LKxUaGUgcXVpY2sgYnJvd24gZm94IGp1bXBzIG92ZXIgMTMgbGF6eSBkb2dzLg==").as_bytes())),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
