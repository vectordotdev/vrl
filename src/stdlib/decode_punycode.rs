use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct DecodePunycode;

impl Function for DecodePunycode {
    fn identifier(&self) -> &'static str {
        "decode_punycode"
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

        Ok(DecodePunycodeFn { value, validate }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "punycode string",
                source: r#"decode_punycode!("www.xn--caf-dma.com")"#,
                result: Ok("www.café.com"),
            },
            Example {
                title: "ascii string",
                source: r#"decode_punycode!("www.cafe.com")"#,
                result: Ok("www.cafe.com"),
            },
            Example {
                title: "ignore validation",
                source: r#"decode_punycode!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib.", validate: false)"#,
                result: Ok("١٠.٦٦.٣٠.٥."),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct DecodePunycodeFn {
    value: Box<dyn Expression>,
    validate: Box<dyn Expression>,
}

impl FunctionExpression for DecodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let validate = self.validate.resolve(ctx)?.try_boolean()?;

        let (encoded, result) = idna::domain_to_unicode(&string);

        if validate {
            result.map_err(|errors| format!("unable to decode punycode: {errors}"))?;
        }

        Ok(encoded.into())
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
            want: Err("unable to decode punycode: Errors { check_bidi }"),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn.")],
            want: Err("unable to decode punycode: Errors { punycode, check_bidi }"),
            tdef: TypeDef::bytes().fallible(),
        }

        bidi_error_ignore {
            args: func_args![value: value!("xn--8hbb.xn--fiba.xn--8hbf.xn--eib."), validate: false],
            want: Ok(value!("١٠.٦٦.٣٠.٥.")),
            tdef: TypeDef::bytes().fallible(),
        }

        multiple_errors_ignore {
            args: func_args![value: value!("dns1.webproxy.idc.csesvcgateway.xn--line-svcgateway-jp-mvm-ri-d060072.\\-1roslin.canva.cn."), validate: false],
            want: Ok(value!("dns1.webproxy.idc.csesvcgateway..\\-1roslin.canva.cn.")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
