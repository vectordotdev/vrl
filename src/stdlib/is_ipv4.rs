use crate::compiler::prelude::*;
use std::net::Ipv4Addr;

fn is_ipv4(value: &Value) -> Resolved {
    let value_str = value.try_bytes_utf8_lossy()?;
    Ok(value_str.parse::<Ipv4Addr>().is_ok().into())
}

#[derive(Clone, Copy, Debug)]
pub struct IsIpv4;

impl Function for IsIpv4 {
    fn identifier(&self) -> &'static str {
        "is_ipv4"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Check if the string is a valid IPv4 address or not.

            An [IPv4-mapped](https://datatracker.ietf.org/doc/html/rfc6890) or
            [IPv4-compatible](https://datatracker.ietf.org/doc/html/rfc6890) IPv6 address is not considered
            valid for the purpose of this function.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The IP address to check",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid IPv4 address",
                source: r#"is_ipv4("10.0.102.37")"#,
                result: Ok("true"),
            },
            example! {
                title: "Valid IPv6 address",
                source: r#"is_ipv4("2001:0db8:85a3:0000:0000:8a2e:0370:7334")"#,
                result: Ok("false"),
            },
            example! {
                title: "Arbitrary string",
                source: r#"is_ipv4("foobar")"#,
                result: Ok("false"),
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

        Ok(IsIpv4Fn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsIpv4Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsIpv4Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).and_then(|v| is_ipv4(&v))
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        is_ipv4 => IsIpv4;

        not_string {
            args: func_args![value: value!(42)],
            want: Err("expected string, got integer"),
            tdef: TypeDef::boolean().infallible(),
        }

        random_string {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        ipv4_address_valid {
            args: func_args![value: value!("1.1.1.1")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        ipv4_address_invalid {
            args: func_args![value: value!("1.1.1.314")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        ipv6_address {
            args: func_args![value: value!("2001:0db8:85a3:0000:0000:8a2e:0370:7334")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
