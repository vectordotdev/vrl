use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_VALIDATE: LazyLock<Value> = LazyLock::new(|| Value::Boolean(true));

const PUNYCODE_PREFIX: &str = "xn--";

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to decode.",
            default: None,
        },
        Parameter {
            keyword: "validate",
            kind: kind::BOOLEAN,
            required: false,
            description: "If enabled, checks if the input string is a valid domain name.",
            default: Some(&DEFAULT_VALIDATE),
        },
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct DecodePunycode;

impl Function for DecodePunycode {
    fn identifier(&self) -> &'static str {
        "decode_punycode"
    }

    fn usage(&self) -> &'static str {
        "Decodes a [punycode](https://en.wikipedia.org/wiki/Punycode) encoded `value`, such as an internationalized domain name ([IDN](https://en.wikipedia.org/wiki/Internationalized_domain_name)). This function assumes that the value passed is meant to be used in IDN context and that it is either a domain name or a part of it."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not valid `punycode`"]
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
        let validate = arguments.optional("validate");

        Ok(DecodePunycodeFn { value, validate }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Decode a punycode encoded internationalized domain name",
                source: r#"decode_punycode!("www.xn--caf-dma.com")"#,
                result: Ok("www.café.com"),
            },
            example! {
                title: "Decode an ASCII only string",
                source: r#"decode_punycode!("www.cafe.com")"#,
                result: Ok("www.cafe.com"),
            },
            example! {
                title: "Ignore validation",
                source: r#"decode_punycode!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.", validate: false)"#,
                result: Ok("١٠.٦٦.٣٠.٥."),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct DecodePunycodeFn {
    value: Box<dyn Expression>,
    validate: Option<Box<dyn Expression>>,
}

impl FunctionExpression for DecodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        if !string.contains(PUNYCODE_PREFIX) {
            return Ok(string.into());
        }

        let validate = self
            .validate
            .map_resolve_with_default(ctx, || DEFAULT_VALIDATE.clone())?
            .try_boolean()?;

        if validate {
            let (decoded, result) = idna::domain_to_unicode(&string);

            result.map_err(|_errors| "unable to decode punycode".to_string())?;
            Ok(decoded.into())
        } else {
            let decoded = string
                .split('.')
                .map(|part| {
                    if let Some(stripped) = part.strip_prefix(PUNYCODE_PREFIX) {
                        idna::punycode::decode_to_string(stripped).unwrap_or(part.to_string())
                    } else {
                        part.to_string()
                    }
                })
                .collect::<Vec<String>>()
                .join(".");
            Ok(decoded.into())
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        decode_punycode => DecodePunycode;

        demo_string {
            args: func_args![value: value!("www.xn--caf-dma.com")],
            want: Ok(value!("www.café.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        ascii_string {
            args: func_args![value: value!("www.cafe.com")],
            want: Ok(value!("www.cafe.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.")],
            want: Err("unable to decode punycode"),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")],
            want: Err("unable to decode punycode"),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error_ignore {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib."), validate: false],
            want: Ok(value!("١٠.٦٦.٣٠.٥.")),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors_ignore {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn."), validate: false],
            want: Ok(value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
