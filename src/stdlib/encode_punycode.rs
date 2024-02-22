use crate::compiler::prelude::*;

const PUNYCODE_PREFIX: &str = "xn--";

#[derive(Clone, Copy, Debug)]
pub struct EncodePunycode;

impl Function for EncodePunycode {
    fn identifier(&self) -> &'static str {
        "encode_punycode"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "validate",
                kind: kind::BOOLEAN,
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
        let validate = arguments
            .optional("validate")
            .unwrap_or_else(|| expr!(true));

        Ok(EncodePunycodeFn { value, validate }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "IDN string",
                source: r#"encode_punycode!("www.café.com")"#,
                result: Ok("www.xn--caf-dma.com"),
            },
            Example {
                title: "mixed case string",
                source: r#"encode_punycode!("www.CAFé.com")"#,
                result: Ok("www.xn--caf-dma.com"),
            },
            Example {
                title: "ascii string",
                source: r#"encode_punycode!("www.cafe.com")"#,
                result: Ok("www.cafe.com"),
            },
            Example {
                title: "ignore validation",
                source: r#"encode_punycode!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.", validate: false)"#,
                result: Ok("xn--8hbb.xn--fiba.xn--8hbf.xn--eib."),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodePunycodeFn {
    value: Box<dyn Expression>,
    validate: Box<dyn Expression>,
}

impl FunctionExpression for EncodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let validate = self.validate.resolve(ctx)?.try_boolean()?;

        if validate {
            let encoded = idna::domain_to_ascii(&string)
                .map_err(|errors| format!("unable to encode to punycode: {errors}"))?;
            Ok(encoded.into())
        } else {
            let encoded = string
                .split('.')
                .map(|part| {
                    if part.starts_with(PUNYCODE_PREFIX) || part.is_ascii() {
                        part.to_string()
                    } else {
                        format!(
                            "{}{}",
                            PUNYCODE_PREFIX,
                            idna::punycode::encode_str(part).unwrap_or(part.to_string())
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

        ascii_string {
            args: func_args![value: value!("www.cafe.com")],
            want: Ok(value!("www.cafe.com")),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.")],
            want: Err("unable to encode to punycode: Errors { check_bidi }"),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")],
            want: Err("unable to encode to punycode: Errors { punycode, check_bidi }"),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error2 {
            args: func_args![value: value!("wwes.ir.abadgostaran.ir.taakads.ir.farhadrahimy.ir.regk.ir.2qok.com.خرید-پستی.com.maskancto.com.phpars.com.eshelstore.ir.techtextile.ir.mrafiei.ir.hamtamotor.com.surfiran.ir.negar3d.com.tjketab.ir.3d4dl.ir.cabindooshsahand.com.mashtikebab.sbs.")],
            want: Err("unable to encode to punycode: Errors { check_bidi }"),
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
