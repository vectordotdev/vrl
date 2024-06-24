use std::net::IpAddr;

use community_id::calculate_community_id;

use crate::compiler::prelude::*;

fn community_id(
    src_ip: Value,
    dst_ip: Value,
    protocol: Value,
    src_port: Option<Value>,
    dst_port: Option<Value>,
    seed: Option<Value>,
) -> Resolved {
    let src_ip: IpAddr = src_ip
        .try_bytes_utf8_lossy()?
        .parse()
        .map_err(|err| format!("unable to parse source IP address: {err}"))?;

    let dst_ip: IpAddr = dst_ip
        .try_bytes_utf8_lossy()?
        .parse()
        .map_err(|err| format!("unable to parse destination IP address: {err}"))?;

    let protocol = u8::try_from(protocol.try_integer()?)
        .map_err(|err| format!("protocol must be between 0 and 255: {err}"))?;

    let src_port = src_port
        .map(VrlValueConvert::try_integer)
        .transpose()?
        .map(|value| {
            u16::try_from(value)
                .map_err(|err| format!("source port must be between 0 and 65535: {err}"))
        })
        .transpose()?;

    let dst_port = dst_port
        .map(VrlValueConvert::try_integer)
        .transpose()?
        .map(|value| {
            u16::try_from(value)
                .map_err(|err| format!("destination port must be between 0 and 65535: {err}"))
        })
        .transpose()?;

    let seed = seed
        .map(VrlValueConvert::try_integer)
        .transpose()?
        .map_or(Ok(0), u16::try_from)
        .map_err(|err| format!("seed must be between 0 and 65535: {err}"))?;

    let id = calculate_community_id(seed, src_ip, dst_ip, src_port, dst_port, protocol, false);

    match id {
        Ok(id) => Ok(Value::Bytes(id.into())),
        Err(err) => Err(ExpressionError::from(err.to_string())),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CommunityID;

impl Function for CommunityID {
    fn identifier(&self) -> &'static str {
        "community_id"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "source_ip",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "destination_ip",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "protocol",
                kind: kind::INTEGER,
                required: true,
            },
            Parameter {
                keyword: "source_port",
                kind: kind::INTEGER,
                required: false,
            },
            Parameter {
                keyword: "destination_port",
                kind: kind::INTEGER,
                required: false,
            },
            Parameter {
                keyword: "seed",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "TCP",
                source: r#"community_id!(source_ip: "1.2.3.4", destination_ip: "5.6.7.8", source_port: 1122, destination_port: 3344, protocol: 6)"#,
                result: Ok("1:wCb3OG7yAFWelaUydu0D+125CLM="),
            },
            Example {
                title: "UDP",
                source: r#"community_id!(source_ip: "1.2.3.4", destination_ip: "5.6.7.8", source_port: 1122, destination_port: 3344, protocol: 17)"#,
                result: Ok("1:0Mu9InQx6z4ZiCZM/7HXi2WMhOg="),
            },
            Example {
                title: "ICMP",
                source: r#"community_id!(source_ip: "1.2.3.4", destination_ip: "5.6.7.8", source_port: 8, destination_port: 0, protocol: 1)"#,
                result: Ok("1:crodRHL2FEsHjbv3UkRrfbs4bZ0="),
            },
            Example {
                title: "RSVP",
                source: r#"community_id!(source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 46)"#,
                result: Ok("1:ikv3kmf89luf73WPz1jOs49S768="),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let src_ip = arguments.required("source_ip");
        let dst_ip = arguments.required("destination_ip");
        let protocol = arguments.required("protocol");
        let src_port = arguments.optional("source_port");
        let dst_port = arguments.optional("destination_port");
        let seed = arguments.optional("seed");

        if let Some(protocol) = protocol.resolve_constant(state) {
            if let Some(protocol_literal) = protocol.as_integer() {
                if u8::try_from(protocol_literal).is_err() {
                    return Err(function::Error::InvalidArgument {
                        keyword: "protocol",
                        value: protocol,
                        error: r#""protocol" must be between 0 and 255"#,
                    }
                    .into());
                }
            }
        }

        if let Some(src_port) = &src_port {
            if let Some(src_port) = src_port.resolve_constant(state) {
                if let Some(src_port_literal) = src_port.as_integer() {
                    if u16::try_from(src_port_literal).is_err() {
                        return Err(function::Error::InvalidArgument {
                            keyword: "source_port",
                            value: src_port,
                            error: r#""source_port" must be between 0 and 65535"#,
                        }
                        .into());
                    }
                }
            }
        }

        if let Some(dst_port) = &dst_port {
            if let Some(dst_port) = dst_port.resolve_constant(state) {
                if let Some(dst_port_literal) = dst_port.as_integer() {
                    if u16::try_from(dst_port_literal).is_err() {
                        return Err(function::Error::InvalidArgument {
                            keyword: "destination_port",
                            value: dst_port,
                            error: r#""destination_port" must be between 0 and 65535"#,
                        }
                        .into());
                    }
                }
            }
        }

        if let Some(seed) = &seed {
            if let Some(seed) = seed.resolve_constant(state) {
                if let Some(seed_literal) = seed.as_integer() {
                    if u16::try_from(seed_literal).is_err() {
                        return Err(function::Error::InvalidArgument {
                            keyword: "seed",
                            value: seed,
                            error: r#""seed" must be between 0 and 65535"#,
                        }
                        .into());
                    }
                }
            }
        }

        Ok(CommunityIDFn {
            src_ip,
            dst_ip,
            protocol,
            src_port,
            dst_port,
            seed,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct CommunityIDFn {
    src_ip: Box<dyn Expression>,
    dst_ip: Box<dyn Expression>,
    protocol: Box<dyn Expression>,
    src_port: Option<Box<dyn Expression>>,
    dst_port: Option<Box<dyn Expression>>,
    seed: Option<Box<dyn Expression>>,
}

impl FunctionExpression for CommunityIDFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let src_ip: Value = self.src_ip.resolve(ctx)?;
        let dst_ip: Value = self.dst_ip.resolve(ctx)?;
        let protocol = self.protocol.resolve(ctx)?;

        let src_port = self
            .src_port
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        let dst_port = self
            .dst_port
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        let seed = self
            .seed
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;

        community_id(src_ip, dst_ip, protocol, src_port, dst_port, seed)
    }

    fn type_def(&self, _state: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        community_id => CommunityID;
        // Examples from https://github.com/corelight/community-id-spec/tree/master/baseline
        tcp_default_seed {
             args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6, source_port: 1122, destination_port: 3344],
             want: Ok("1:wCb3OG7yAFWelaUydu0D+125CLM="),
             tdef: TypeDef::bytes().fallible(),
        }

        tcp_reverse_default_seed {
            args: func_args![source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: 6, source_port: 3344, destination_port: 1122],
            want: Ok("1:wCb3OG7yAFWelaUydu0D+125CLM="),
            tdef: TypeDef::bytes().fallible(),
       }

        tcp_no_ports {
            args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6],
            want: Err("src port and dst port should be set when protocol is tcp/udp/sctp"),
            tdef: TypeDef::bytes().fallible(),
        }

        tcp_source_port_too_large {
            args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6, source_port: u64::MAX, destination_port: 80],
            want: Err("invalid argument"),
            tdef: TypeDef::bytes().fallible(),
        }

        tcp_destination_port_too_large {
            args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6, source_port: 80 , destination_port: u64::MAX],
            want: Err("invalid argument"),
            tdef: TypeDef::bytes().fallible(),
        }

        udp_default_seed {
            args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 17, source_port: 1122, destination_port: 3344],
            want: Ok("1:0Mu9InQx6z4ZiCZM/7HXi2WMhOg="),
            tdef: TypeDef::bytes().fallible(),
       }

        udp_reverse_default_seed {
            args: func_args![source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: 17, source_port: 3344, destination_port: 1122],
            want: Ok("1:0Mu9InQx6z4ZiCZM/7HXi2WMhOg="),
            tdef: TypeDef::bytes().fallible(),
        }

        rsvp_default_seed {
            args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 46],
            want: Ok("1:ikv3kmf89luf73WPz1jOs49S768="),
            tdef: TypeDef::bytes().fallible(),
        }

        rsvp_reverse_default_seed {
            args: func_args![source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: 46],
            want: Ok("1:ikv3kmf89luf73WPz1jOs49S768="),
            tdef: TypeDef::bytes().fallible(),
        }

        tcp_seed_1 {
            args: func_args![seed: 1, source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6, source_port: 1122, destination_port: 3344],
            want: Ok("1:HhA1B+6CoLbiKPEs5nhNYN4XWfk="),
            tdef: TypeDef::bytes().fallible(),
        }

        tcp_reverse_seed_1 {
            args: func_args![seed: 1,source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: 6, source_port: 3344, destination_port: 1122],
            want: Ok("1:HhA1B+6CoLbiKPEs5nhNYN4XWfk="),
            tdef: TypeDef::bytes().fallible(),
        }

        seed_too_large {
            args: func_args![seed: u64::MAX,source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: 6, source_port: 3344, destination_port: 1122],
            want: Err("invalid argument"),
            tdef: TypeDef::bytes().fallible(),
        }

        protocol_too_large {
            args: func_args![source_ip: "5.6.7.8", destination_ip: "1.2.3.4", protocol: i64::MAX, source_port: 3344, destination_port: 1122],
            want: Err("invalid argument"),
            tdef: TypeDef::bytes().fallible(),
        }

    ];
}
