use crate::compiler::prelude::*;
use nom::AsBytes;
use std::sync::LazyLock;

static DEFAULT_COMPRESSION_LEVEL: LazyLock<Value> = LazyLock::new(|| Value::Integer(3));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to encode.",
            default: None,
        },
        Parameter {
            keyword: "compression_level",
            kind: kind::INTEGER,
            required: false,
            description: "The default compression level.",
            default: Some(&DEFAULT_COMPRESSION_LEVEL),
        },
    ]
});

fn encode_zstd(value: Value, compression_level: Value) -> Resolved {
    #[allow(clippy::cast_possible_truncation)] //TODO evaluate removal options
    let compression_level = compression_level.try_integer()? as i32;

    let value = value.try_bytes()?;
    // Zstd encoding will not fail in the case of using `encode_all` function
    let encoded_bytes = zstd::encode_all(value.as_bytes(), compression_level)
        .expect("zstd compression failed, please report");

    Ok(Value::Bytes(encoded_bytes.into()))
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeZstd;

impl Function for EncodeZstd {
    fn identifier(&self) -> &'static str {
        "encode_zstd"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to [Zstandard](https://facebook.github.io/zstd)."
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Encode to Zstd",
            source: r#"encode_base64(encode_zstd("please encode me"))"#,
            result: Ok("KLUv/QBYgQAAcGxlYXNlIGVuY29kZSBtZQ=="),
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

        Ok(EncodeZstdFn {
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
struct EncodeZstdFn {
    value: Box<dyn Expression>,
    compression_level: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeZstdFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        let compression_level = self
            .compression_level
            .map_resolve_with_default(ctx, || DEFAULT_COMPRESSION_LEVEL.clone())?;

        encode_zstd(value, compression_level)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}
