use crate::diagnostic::Label;
use crate::prelude::*;
use encoding_rs::Encoding;
use nom::AsBytes;
use std::str::from_utf8;

#[derive(Clone, Copy, Debug)]
pub struct EncodeCharset;

impl Function for EncodeCharset {
    fn identifier(&self) -> &'static str {
        "encode_charset"
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Encode charset to euc-kr",
                source: r#"encode_base64(encode_charset!("안녕하세요", "euc-kr"))"#,
                result: Ok("vsiz58fPvLy/5A=="),
            },
            Example {
                title: "Encode charset to euc-jp",
                source: r#"encode_base64(encode_charset!("こんにちは", "euc-jp"))"#,
                result: Ok(r"pLOk86TLpMGkzw=="),
            },
            Example {
                title: "Encode charset to gb2312",
                source: r#"encode_base64(encode_charset!("你好", "gb2312"))"#,
                result: Ok(r"xOO6ww=="),
            },
        ]
    }

    fn summary(&self) -> &'static str {
        "Encode UTF-8 to non UTF-8 charset"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Encode UTF-8 to non UTF-8 charset.

            The `value` parameter is a UTF-8 encoded string.
            The `to_charset` parameter specifies the charset to encode the `value`.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "to_charset",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let to_charset = arguments.required("to_charset");

        Ok(DecodeCharsetFn { value, to_charset }.as_expr())
    }
}

fn encode_charset(value: &str, to_charset: &[u8]) -> Resolved {
    let encoder = Encoding::for_label(to_charset).ok_or_else(|| create_error(to_charset))?;

    let (output, _, _) = encoder.encode(value);
    Ok(Value::Bytes(output.as_bytes().to_vec().into()))
}

fn create_error(to_charset: &[u8]) -> ExpressionError {
    ExpressionError::Error {
        message: format!(
            "Unknown charset: {}",
            from_utf8(to_charset).unwrap_or("unknown")
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
    to_charset: Box<dyn Expression>,
}

impl FunctionExpression for DecodeCharsetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?.try_bytes()?;
        let to_charset = self.to_charset.resolve(ctx)?.try_bytes()?;

        encode_charset(from_utf8(value.as_bytes()).unwrap(), to_charset.as_bytes())
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
        encode_charset => EncodeCharset;

        encode_to_euc_kr {
            args: func_args![value: value!("안녕하세요"),
                             to_charset: value!("euc-kr")],
            want: Ok(value!(b"\xbe\xc8\xb3\xe7\xc7\xcf\xbc\xbc\xbf\xe4")),
            tdef: TypeDef::bytes().fallible(),
        }

        encode_to_euc_jp {
            args: func_args![value: value!("こんにちは"),
                             to_charset: value!("euc-jp")],
            want: Ok(value!(b"\xa4\xb3\xa4\xf3\xa4\xcb\xa4\xc1\xa4\xcf")),
            tdef: TypeDef::bytes().fallible(),
        }

        encode_to_gb2312 {
            args: func_args![value: value!("你好"),
                             to_charset: value!("gb2312")],
            want: Ok(value!(b"\xc4\xe3\xba\xc3")),
            tdef: TypeDef::bytes().fallible(),
        }

        unknown_charset {
                args: func_args![value: value!("안녕하세요"),
                             to_charset: value!("euc--kr")],
            want: Err("Unknown charset: euc--kr"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
