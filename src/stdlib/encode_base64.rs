use crate::compiler::prelude::*;
use crate::stdlib::util::Base64Charset;
use std::sync::LazyLock;

static DEFAULT_PADDING: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));
static DEFAULT_CHARSET: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("standard")));

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
            keyword: "padding",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether the Base64 output is [padded](https://en.wikipedia.org/wiki/Base64#Output_padding).",
            default: Some(&DEFAULT_PADDING),
        },
        Parameter {
            keyword: "charset",
            kind: kind::BYTES,
            required: false,
            description: "The character set to use when encoding the data.",
            default: Some(&DEFAULT_CHARSET),
        },
    ]
});

fn encode_base64(value: Value, padding: Value, charset: Value) -> Resolved {
    let value = value.try_bytes()?;
    let padding = padding.try_boolean()?;
    let charset = Base64Charset::from_slice(&charset.try_bytes()?)?;

    let encoder = match (padding, charset) {
        (true, Base64Charset::Standard) => base64_simd::STANDARD,
        (false, Base64Charset::Standard) => base64_simd::STANDARD_NO_PAD,
        (true, Base64Charset::UrlSafe) => base64_simd::URL_SAFE,
        (false, Base64Charset::UrlSafe) => base64_simd::URL_SAFE_NO_PAD,
    };

    Ok(encoder.encode_to_string(value).into())
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeBase64;

impl Function for EncodeBase64 {
    fn identifier(&self) -> &'static str {
        "encode_base64"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to [Base64](https://en.wikipedia.org/wiki/Base64)."
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let padding = arguments.optional("padding");
        let charset = arguments.optional("charset");

        Ok(EncodeBase64Fn {
            value,
            padding,
            charset,
        }
        .as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Encode to Base64 (default)",
                source: r#"encode_base64("please encode me")"#,
                result: Ok("cGxlYXNlIGVuY29kZSBtZQ=="),
            },
            example! {
                title: "Encode to Base64 (without padding)",
                source: r#"encode_base64("please encode me, no padding though", padding: false)"#,
                result: Ok("cGxlYXNlIGVuY29kZSBtZSwgbm8gcGFkZGluZyB0aG91Z2g"),
            },
            example! {
                title: "Encode to Base64 (URL safe)",
                source: r#"encode_base64("please encode me, but safe for URLs", charset: "url_safe")"#,
                result: Ok("cGxlYXNlIGVuY29kZSBtZSwgYnV0IHNhZmUgZm9yIFVSTHM="),
            },
            example! {
                title: "Encode to Base64 (without padding and URL safe)",
                source: r#"encode_base64("some string value", padding: false, charset: "url_safe")"#,
                result: Ok("c29tZSBzdHJpbmcgdmFsdWU"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodeBase64Fn {
    value: Box<dyn Expression>,
    padding: Option<Box<dyn Expression>>,
    charset: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodeBase64Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let padding = self
            .padding
            .map_resolve_with_default(ctx, || DEFAULT_PADDING.clone())?;
        let charset = self
            .charset
            .map_resolve_with_default(ctx, || DEFAULT_CHARSET.clone())?;

        encode_base64(value, padding, charset)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        encode_base64 => EncodeBase64;

        with_defaults {
            args: func_args![value: value!("some+=string/value")],
            want: Ok(value!("c29tZSs9c3RyaW5nL3ZhbHVl")),
            tdef: TypeDef::bytes().infallible(),
        }

        with_padding_standard_charset {
            args: func_args![value: value!("some+=string/value"), padding: value!(true), charset: value!("standard")],
            want: Ok(value!("c29tZSs9c3RyaW5nL3ZhbHVl")),
            tdef: TypeDef::bytes().infallible(),
        }

        no_padding_standard_charset {
            args: func_args![value: value!("some+=string/value"), padding: value!(false), charset: value!("standard")],
            want: Ok(value!("c29tZSs9c3RyaW5nL3ZhbHVl")),
            tdef: TypeDef::bytes().infallible(),
        }

        with_padding_urlsafe_charset {
            args: func_args![value: value!("some+=string/value"), padding: value!(true), charset: value!("url_safe")],
            want: Ok(value!("c29tZSs9c3RyaW5nL3ZhbHVl")),
            tdef: TypeDef::bytes().infallible(),
        }

        no_padding_urlsafe_charset {
            args: func_args![value: value!("some+=string/value"), padding: value!(false), charset: value!("url_safe")],
            want: Ok(value!("c29tZSs9c3RyaW5nL3ZhbHVl")),
            tdef: TypeDef::bytes().infallible(),
        }

        with_padding_standard_charset_unicode {
            args: func_args![value: value!("some=string/řčža"), padding: value!(true), charset: value!("standard")],
            want: Ok(value!("c29tZT1zdHJpbmcvxZnEjcW+YQ==")),
            tdef: TypeDef::bytes().infallible(),
        }

        no_padding_standard_charset_unicode {
            args: func_args![value: value!("some=string/řčža"), padding: value!(false), charset: value!("standard")],
            want: Ok(value!("c29tZT1zdHJpbmcvxZnEjcW+YQ")),
            tdef: TypeDef::bytes().infallible(),
        }

        with_padding_urlsafe_charset_unicode {
            args: func_args![value: value!("some=string/řčža"), padding: value!(true), charset: value!("url_safe")],
            want: Ok(value!("c29tZT1zdHJpbmcvxZnEjcW-YQ==")),
            tdef: TypeDef::bytes().infallible(),
        }

        no_padding_urlsafe_charset_unicode {
            args: func_args![value: value!("some=string/řčža"), padding: value!(false), charset: value!("url_safe")],
            want: Ok(value!("c29tZT1zdHJpbmcvxZnEjcW-YQ")),
            tdef: TypeDef::bytes().infallible(),
        }

        empty_string_standard_charset {
            args: func_args![value: value!(""), charset: value!("standard")],
            want: Ok(value!("")),
            tdef: TypeDef::bytes().infallible(),
        }

        empty_string_urlsafe_charset {
            args: func_args![value: value!(""), charset: value!("url_safe")],
            want: Ok(value!("")),
            tdef: TypeDef::bytes().infallible(),
        }

        invalid_charset_error {
            args: func_args![value: value!("some string value"), padding: value!(false), charset: value!("foo")],
            want: Err("unknown charset"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
