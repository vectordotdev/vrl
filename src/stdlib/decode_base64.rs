use crate::compiler::prelude::*;
use crate::stdlib::util::Base64Charset;

fn decode_base64(charset: Option<Value>, value: Value) -> Resolved {
    let value = value.try_bytes()?;
    let charset = charset
        .map(Value::try_bytes)
        .transpose()?
        .as_deref()
        .map(Base64Charset::from_slice)
        .transpose()?
        .unwrap_or_default();

    let decoder = match charset {
        Base64Charset::Standard => base64_simd::STANDARD_NO_PAD,
        Base64Charset::UrlSafe => base64_simd::URL_SAFE_NO_PAD,
    };

    // Find the position of padding char '='
    let pos = value
        .iter()
        .rev()
        .position(|c| *c != b'=')
        .map_or(value.len(), |p| value.len() - p);

    let decoded_vec = decoder
        .decode_to_vec(&value[0..pos])
        .map_err(|_| "unable to decode value from base64")?;

    Ok(Value::Bytes(Bytes::from(decoded_vec)))
}

#[derive(Clone, Copy, Debug)]
pub struct DecodeBase64;

impl Function for DecodeBase64 {
    fn identifier(&self) -> &'static str {
        "decode_base64"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "charset",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let charset = arguments.optional("charset");

        Ok(DecodeBase64Fn { value, charset }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "demo string",
            source: r#"decode_base64!("c29tZSBzdHJpbmcgdmFsdWU=")"#,
            result: Ok("some string value"),
        }]
    }
}

#[derive(Clone, Debug)]
struct DecodeBase64Fn {
    value: Box<dyn Expression>,
    charset: Option<Box<dyn Expression>>,
}

impl FunctionExpression for DecodeBase64Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let charset = self.charset.as_ref().map(|c| c.resolve(ctx)).transpose()?;

        decode_base64(charset, value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        // Always fallible due to the possibility of decoding errors that VRL can't detect in
        // advance: https://docs.rs/base64/0.13.0/base64/enum.DecodeError.html
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        decode_base64 => DecodeBase64;

        with_defaults {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVl")],
            want: Ok(value!("some+=string/value")),
            tdef: TypeDef::bytes().fallible(),
        }

        with_standard_charset {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVl"), charset: value!["standard"]],
            want: Ok(value!("some+=string/value")),
            tdef: TypeDef::bytes().fallible(),
        }

        with_urlsafe_charset {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVl"), charset: value!("url_safe")],
            want: Ok(value!("some+=string/value")),
            tdef: TypeDef::bytes().fallible(),
        }

        with_invalid_charset {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVl"), charset: value!("invalid")],
            want: Err("unknown charset"),
            tdef: TypeDef::bytes().fallible(),
        }

        with_defaults_invalid_value {
            args: func_args![value: value!("helloworld")],
            want: Err("unable to decode value from base64"),
            tdef: TypeDef::bytes().fallible(),
        }

        empty_string_standard_charset {
            args: func_args![value: value!(""), charset: value!("standard")],
            want: Ok(value!("")),
            tdef: TypeDef::bytes().fallible(),
        }

        empty_string_urlsafe_charset {
            args: func_args![value: value!(""), charset: value!("url_safe")],
            want: Ok(value!("")),
            tdef: TypeDef::bytes().fallible(),
        }

        // decode_base64 function should be able to decode base64 string with or without padding
        padding_not_included {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVlXw")],
            want: Ok(value!("some+=string/value_")),
            tdef: TypeDef::bytes().fallible(),
        }

        padding_included {
            args: func_args![value: value!("c29tZSs9c3RyaW5nL3ZhbHVlXw==")],
            want: Ok(value!("some+=string/value_")),
            tdef: TypeDef::bytes().fallible(),
        }

        // https://github.com/vectordotdev/vrl/issues/959
        no_padding {
            args: func_args![value: value!("eyJzY2hlbWEiOiJpZ2x1OmNvbS5zbm93cGxvd2FuYWx5dGljcy5zbm93cGxvdy91bnN0cnVjdF9ldmVudC9qc29uc2NoZW1hLzEtMC0wIiwiZGF0YSI6eyJzY2hlbWEiOiJpZ2x1OmNvbS5zbm93cGxvd2FuYWx5dGljcy5zbm93cGxvdy9saW5rX2NsaWNrL2pzb25zY2hlbWEvMS0wLTEiLCJkYXRhIjp7InRhcmdldFVybCI6Imh0dHBzOi8vaWRwLWF1dGguZ2FyLmVkdWNhdGlvbi5mci9kb21haW5lR2FyP2lkRU5UPVNqQT0maWRTcmM9WVhKck9pODBPRFUyTmk5d2RERTRNREF3TVE9PSIsImVsZW1lbnRJZCI6IiIsImVsZW1lbnRDbGFzc2VzIjpbImxpbmstYnV0dG9uIiwidHJhY2tlZCJdLCJlbGVtZW50VGFyZ2V0IjoiX2JsYW5rIn19fQ")],
            want: Ok(value!(r#"{"schema":"iglu:com.snowplowanalytics.snowplow/unstruct_event/jsonschema/1-0-0","data":{"schema":"iglu:com.snowplowanalytics.snowplow/link_click/jsonschema/1-0-1","data":{"targetUrl":"https://idp-auth.gar.education.fr/domaineGar?idENT=SjA=&idSrc=YXJrOi80ODU2Ni9wdDE4MDAwMQ==","elementId":"","elementClasses":["link-button","tracked"],"elementTarget":"_blank"}}}"#)),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
