use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct DecodePunycode;

impl Function for DecodePunycode {
    fn identifier(&self) -> &'static str {
        "decode_punycode"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(DecodePunycodeFn { value }.as_expr())
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
        ]
    }
}

#[derive(Clone, Debug)]
struct DecodePunycodeFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DecodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let (encoded, result) = idna::domain_to_unicode(&string);
        result.map_err(|errors| format!("unable to decode punycode: {errors}"))?;

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
    ];
}
