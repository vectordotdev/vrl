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
            description: "The string to encode.",
            default: None,
        },
        Parameter {
            keyword: "validate",
            kind: kind::BOOLEAN,
            required: false,
            description: "Whether to validate the input string to check if it is a valid domain name.",
            default: Some(&DEFAULT_VALIDATE),
        },
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct EncodePunycode;

impl Function for EncodePunycode {
    fn identifier(&self) -> &'static str {
        "encode_punycode"
    }

    fn usage(&self) -> &'static str {
        "Encodes a `value` to [punycode](https://en.wikipedia.org/wiki/Punycode). Useful for internationalized domain names ([IDN](https://en.wikipedia.org/wiki/Internationalized_domain_name)). This function assumes that the value passed is meant to be used in IDN context and that it is either a domain name or a part of it."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` can not be encoded to `punycode`"]
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
        let validate = arguments.optional("validate");

        Ok(EncodePunycodeFn { value, validate }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Encode an internationalized domain name",
                source: r#"encode_punycode!("www.café.com")"#,
                result: Ok("www.xn--caf-dma.com"),
            },
            example! {
                title: "Encode an internationalized domain name with mixed case",
                source: r#"encode_punycode!("www.CAFé.com")"#,
                result: Ok("www.xn--caf-dma.com"),
            },
            example! {
                title: "Encode an ASCII only string",
                source: r#"encode_punycode!("www.cafe.com")"#,
                result: Ok("www.cafe.com"),
            },
            example! {
                title: "Ignore validation",
                source: r#"encode_punycode!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.", validate: false)"#,
                result: Ok("xn--8hbb.xn--fiba.xn--8hbf.xn--eib."),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodePunycodeFn {
    value: Box<dyn Expression>,
    validate: Option<Box<dyn Expression>>,
}

impl FunctionExpression for EncodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let validate = self
            .validate
            .map_resolve_with_default(ctx, || DEFAULT_VALIDATE.clone())?
            .try_boolean()?;

        if validate {
            let encoded = idna::domain_to_ascii(&string)
                .map_err(|_errors| "unable to encode to punycode".to_string())?;
            Ok(encoded.into())
        } else {
            if string
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')
            {
                return Ok(string.into());
            }

            let encoded = string
                .split('.')
                .map(|part| {
                    if part.starts_with(PUNYCODE_PREFIX) || part.is_ascii() {
                        part.to_lowercase()
                    } else {
                        format!(
                            "{}{}",
                            PUNYCODE_PREFIX,
                            idna::punycode::encode_str(&part.to_lowercase())
                                .unwrap_or(part.to_lowercase())
                        )
                    }
                })
                .collect::<Vec<String>>()
                .join(".");
            Ok(encoded.into())
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
        encode_punycode => EncodePunycode;

        idn_string {
            args: func_args![value: value!("www.café.com")],
            want: Ok(value!("www.xn--caf-dma.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        mixed_case {
            args: func_args![value: value!("www.CAFé.com")],
            want: Ok(value!("www.xn--caf-dma.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        mixed_case_ignore_validation {
            args: func_args![value: value!("www.CAFé.com"), validate: false],
            want: Ok(value!("www.xn--caf-dma.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        ascii_string {
            args: func_args![value: value!("www.cafe.com")],
            want: Ok(value!("www.cafe.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        ascii_string_ignore_validation {
            args: func_args![value: value!("www.cafe.com"), validate: false],
            want: Ok(value!("www.cafe.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.")],
            want: Err("unable to encode to punycode"),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")],
            want: Err("unable to encode to punycode"),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error2 {
            args: func_args![value: value!("wwes.ir.abadgostaran.ir.taakads.ir.farhadrahimy.ir.regk.ir.2qok.com.خرید-پستی.com.maskancto.com.phpars.com.eshelstore.ir.techtextile.ir.mrafiei.ir.hamtamotor.com.surfiran.ir.negar3d.com.tjketab.ir.3d4dl.ir.cabindooshsahand.com.mashtikebab.sbs.")],
            want: Err("unable to encode to punycode"),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error_ignore {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib."), validate: false],
            want: Ok(value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.")),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error2_ignore {
            args: func_args![value: value!("wwes.ir.abadgostaran.ir.taakads.ir.farhadrahimy.ir.regk.ir.2qok.com.خرید-پستی.com.maskancto.com.phpars.com.eshelstore.ir.techtextile.ir.mrafiei.ir.hamtamotor.com.surfiran.ir.negar3d.com.tjketab.ir.3d4dl.ir.cabindooshsahand.com.mashtikebab.sbs."), validate: false],
            want: Ok(value!("wwes.ir.abadgostaran.ir.taakads.ir.farhadrahimy.ir.regk.ir.2qok.com.xn----5mckejo83c6tfa.com.maskancto.com.phpars.com.eshelstore.ir.techtextile.ir.mrafiei.ir.hamtamotor.com.surfiran.ir.negar3d.com.tjketab.ir.3d4dl.ir.cabindooshsahand.com.mashtikebab.sbs.")),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors_ignore {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn."), validate: false],
            want: Ok(value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
