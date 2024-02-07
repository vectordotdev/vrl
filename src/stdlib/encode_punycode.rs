use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct EncodePunycode;

impl Function for EncodePunycode {
    fn identifier(&self) -> &'static str {
        "encode_punycode"
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

        Ok(EncodePunycodeFn { value }.as_expr())
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
        ]
    }
}

#[derive(Clone, Debug)]
struct EncodePunycodeFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodePunycodeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let string = value.try_bytes_utf8_lossy()?;

        let encoded = idna::domain_to_ascii(&string)
            .map_err(|errors| format!("unable to encode to punycode: {errors}"))?;

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
    ];
}
