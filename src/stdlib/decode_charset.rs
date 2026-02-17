use crate::diagnostic::Label;
use crate::prelude::*;
use encoding_rs::Encoding;
use nom::AsBytes;
use std::str::from_utf8;

#[derive(Clone, Copy, Debug)]
pub struct DecodeCharset;

impl Function for DecodeCharset {
    fn identifier(&self) -> &'static str {
        "decode_charset"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Decode EUC-KR string",
                source: r#"decode_charset!(decode_base64!("vsiz58fPvLy/5A=="), "euc-kr")"#,
                result: Ok("안녕하세요"),
            },
            example! {
                title: "Decode EUC-JP string",
                source: r#"decode_charset!(decode_base64!("pLOk86TLpMGkzw=="), "euc-jp")"#,
                result: Ok("こんにちは"),
            },
            example! {
                title: "Decode GB2312 string",
                source: r#"decode_charset!(decode_base64!("xOO6ww=="), "gb2312")"#,
                result: Ok("你好"),
            },
        ]
    }

    fn summary(&self) -> &'static str {
        "Decode non UTF-8 charset to UTF-8"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Decode non UTF-8 charset to UTF-8.

            The `value` parameter is a non UTF-8 encoded string.
            The `from_charset` parameter specifies the charset of the `value`.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`from_charset` isn't a valid [character set](https://encoding.spec.whatwg.org/#names-and-labels).",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required("value", kind::BYTES, "The non-UTF8 string to decode."),
            Parameter::required(
                "from_charset",
                kind::BYTES,
                "The [character set](https://encoding.spec.whatwg.org/#names-and-labels) to use when decoding the data.",
            ),
        ];
        PARAMETERS
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let from_charset = arguments.required("from_charset");

        Ok(DecodeCharsetFn {
            value,
            from_charset,
        }
        .as_expr())
    }
}

fn decode_charset(value: &[u8], from_charset: &[u8]) -> Resolved {
    let decoder = Encoding::for_label(from_charset).ok_or_else(|| create_error(from_charset))?;

    let (output, _, _) = decoder.decode(value);
    Ok(Value::Bytes(output.as_bytes().to_vec().into()))
}

fn create_error(from_charset: &[u8]) -> ExpressionError {
    ExpressionError::Error {
        message: format!(
            "Unknown charset: {}",
            from_utf8(from_charset).unwrap_or("unknown")
        ),
        labels: vec![Label::primary("Unknown charset", Span::default())],
        notes: vec![Note::SeeDocs(
            "Encoding Living Standard".to_string(),
            "https://encoding.spec.whatwg.org/".to_string(),
        )],
    }
}

#[derive(Debug, Clone)]
struct DecodeCharsetFn {
    value: Box<dyn Expression>,
    from_charset: Box<dyn Expression>,
}

impl FunctionExpression for DecodeCharsetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?.try_bytes()?;
        let from = self.from_charset.resolve(ctx)?.try_bytes()?;

        decode_charset(value.as_bytes(), from.as_bytes())
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        decode_charset => DecodeCharset;

        decode_from_euc_kr {
            args: func_args![value: b"\xbe\xc8\xb3\xe7\xc7\xcf\xbc\xbc\xbf\xe4",
                             from_charset: value!("euc-kr")],
            want: Ok(value!("안녕하세요")),
            tdef: TypeDef::bytes().fallible(),
        }

        decode_from_euc_jp {
            args: func_args![value: b"\xa4\xb3\xa4\xf3\xa4\xcb\xa4\xc1\xa4\xcf",
                             from_charset: value!("euc-jp")],
            want: Ok(value!("こんにちは")),
            tdef: TypeDef::bytes().fallible(),
        }

        decode_from_gb2312 {
            args: func_args![value: b"\xc4\xe3\xba\xc3",
                             from_charset: value!("gb2312")],
            want: Ok(value!("你好")),
            tdef: TypeDef::bytes().fallible(),
        }

        unknown_charset {
            args: func_args![value: value!(b"\xbe\xc8\xb3\xe7\xc7\xcf\xbc\xbc\xbf\xe4"),
                             from_charset: value!(b"euc--kr")],
            want: Err("Unknown charset: euc--kr"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
