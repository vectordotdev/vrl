use crate::compiler::prelude::*;
use cidr::IpCidr;
use std::net::IpAddr;
use std::str::FromStr;

fn str_to_cidr(v: &str) -> Result<IpCidr, String> {
    IpCidr::from_str(v).map_err(|err| format!("unable to parse CIDR: {err}"))
}

fn value_to_cidr(value: &Value) -> Result<IpCidr, function::Error> {
    let str = &value.as_str().ok_or(function::Error::InvalidArgument {
        keyword: "ip_cidr_contains",
        value: value.clone(),
        error: r#""cidr" must be string"#,
    })?;

    str_to_cidr(str).map_err(|_| function::Error::InvalidArgument {
        keyword: "ip_cidr_contains",
        value: value.clone(),
        error: r#""cidr" must be valid cidr"#,
    })
}

fn ip_cidr_contains(value: &Value, cidr: &Value) -> Resolved {
    let bytes = value.try_bytes_utf8_lossy()?;
    let ip_addr =
        IpAddr::from_str(&bytes).map_err(|err| format!("unable to parse IP address: {err}"))?;

    match cidr {
        Value::Bytes(v) => {
            let cidr = str_to_cidr(&String::from_utf8_lossy(v))?;
            Ok(cidr.contains(&ip_addr).into())
        }
        Value::Array(vec) => {
            for v in vec {
                let cidr = str_to_cidr(&v.try_bytes_utf8_lossy()?)?;
                if cidr.contains(&ip_addr) {
                    return Ok(true.into());
                }
            }
            Ok(false.into())
        }
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::bytes() | Kind::array(Collection::any()),
        }
        .into()),
    }
}

fn ip_cidr_contains_constant(value: &Value, cidr_vec: &[IpCidr]) -> Resolved {
    let bytes = value.try_bytes_utf8_lossy()?;
    let ip_addr =
        IpAddr::from_str(&bytes).map_err(|err| format!("unable to parse IP address: {err}"))?;

    Ok(cidr_vec.iter().any(|cidr| cidr.contains(&ip_addr)).into())
}

#[derive(Clone, Copy, Debug)]
pub struct IpCidrContains;

impl Function for IpCidrContains {
    fn identifier(&self) -> &'static str {
        "ip_cidr_contains"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "cidr",
                kind: kind::BYTES | kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "in range",
                source: r#"ip_cidr_contains!("192.168.0.0/16", "192.168.0.1")"#,
                result: Ok("true"),
            },
            Example {
                title: "not in range",
                source: r#"ip_cidr_contains!("192.168.0.0/24", "192.168.10.32")"#,
                result: Ok("false"),
            },
            Example {
                title: "invalid address",
                source: r#"ip_cidr_contains!("192.168.0.0/24", "INVALID")"#,
                result: Err(
                    r#"function call error for "ip_cidr_contains" at (0:46): unable to parse IP address: invalid IP address syntax"#,
                ),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let cidr = arguments.required("cidr");

        let cidr = match cidr.resolve_constant(state) {
            None => IpCidrType::Expression(cidr),
            Some(value) => IpCidrType::Constant(match value {
                Value::Bytes(_) => vec![value_to_cidr(&value)?],
                Value::Array(vec) => {
                    let mut output = Vec::with_capacity(vec.len());
                    for value in vec {
                        output.push(value_to_cidr(&value)?);
                    }
                    output
                }
                _ => {
                    return Err(function::Error::InvalidArgument {
                        keyword: "ip_cidr_contains",
                        value,
                        error: r#""cidr" must be string or array of strings"#,
                    }
                    .into())
                }
            }),
        };

        let value = arguments.required("value");

        Ok(IpCidrContainsFn { cidr, value }.as_expr())
    }
}

#[derive(Debug, Clone)]
enum IpCidrType {
    Constant(Vec<IpCidr>),
    Expression(Box<dyn Expression>),
}

#[derive(Debug, Clone)]
struct IpCidrContainsFn {
    cidr: IpCidrType,
    value: Box<dyn Expression>,
}

impl FunctionExpression for IpCidrContainsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        match &self.cidr {
            IpCidrType::Constant(cidr_vec) => ip_cidr_contains_constant(&value, cidr_vec),
            IpCidrType::Expression(exp) => {
                let cidr = exp.resolve(ctx)?;
                ip_cidr_contains(&value, &cidr)
            }
        }
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function! [
        ip_cidr_contains => IpCidrContains;

        ipv4_yes {
            args: func_args![value: "192.168.10.32",
                             cidr: "192.168.0.0/16",
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv4_no {
            args: func_args![value: "192.168.10.32",
                             cidr: "192.168.0.0/24",
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv4_yes_array {
            args: func_args![value: "192.168.10.32",
                             cidr: vec!["10.0.0.0/8", "192.168.0.0/16"],
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv4_no_array {
            args: func_args![value: "192.168.10.32",
                             cidr: vec!["10.0.0.0/8", "192.168.0.0/24"],
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv6_yes {
            args: func_args![value: "2001:4f8:3:ba:2e0:81ff:fe22:d1f1",
                             cidr: "2001:4f8:3:ba::/64",
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv6_no {
            args: func_args![value: "2001:4f8:3:ba:2e0:81ff:fe22:d1f1",
                             cidr: "2001:4f8:4:ba::/64",
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv6_yes_array {
            args: func_args![value: "2001:4f8:3:ba:2e0:81ff:fe22:d1f1",
                             cidr: vec!["fc00::/7", "2001:4f8:3:ba::/64"],
            ],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        ipv6_no_array {
            args: func_args![value: "2001:4f8:3:ba:2e0:81ff:fe22:d1f1",
                             cidr: vec!["fc00::/7", "2001:4f8:4:ba::/64"],
            ],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().fallible(),
        }
    ];
}
