use crate::compiler::prelude::*;
use crate::stdlib::ip_utils::to_key;
use ipcrypt_rs::{Ipcrypt, IpcryptPfx};
use std::net::IpAddr;

fn decrypt_ip(ip: &Value, key: Value, mode: &Value) -> Resolved {
    let ip_str = ip.try_bytes_utf8_lossy()?;
    let ip_addr: IpAddr = ip_str
        .parse()
        .map_err(|err| format!("unable to parse IP address: {err}"))?;

    let mode_str = mode.try_bytes_utf8_lossy()?;

    let ip_ver_label = match ip_addr {
        IpAddr::V4(_) => "IPv4",
        IpAddr::V6(_) => "IPv6",
    };

    let decrypted_ip = match mode_str.as_ref() {
        "aes128" => match ip_addr {
            IpAddr::V4(ipv4) => {
                let key = to_key::<16>(key, "aes128", ip_ver_label)?;
                let ipcrypt = Ipcrypt::new(key);
                ipcrypt.decrypt_ipaddr(IpAddr::V4(ipv4))
            }
            IpAddr::V6(ipv6) => {
                let key = to_key::<16>(key, "aes128", ip_ver_label)?;
                let ipcrypt = Ipcrypt::new(key);
                ipcrypt.decrypt_ipaddr(IpAddr::V6(ipv6))
            }
        },
        "pfx" => match ip_addr {
            IpAddr::V4(ipv4) => {
                let key = to_key::<32>(key, "pfx", ip_ver_label)?;
                let ipcrypt_pfx = IpcryptPfx::new(key);
                ipcrypt_pfx.decrypt_ipaddr(IpAddr::V4(ipv4))
            }
            IpAddr::V6(ipv6) => {
                let key = to_key::<32>(key, "pfx", ip_ver_label)?;
                let ipcrypt_pfx = IpcryptPfx::new(key);
                ipcrypt_pfx.decrypt_ipaddr(IpAddr::V6(ipv6))
            }
        },
        _ => {
            return Err(format!("Invalid mode '{mode_str}'. Must be 'aes128' or 'pfx'").into());
        }
    };

    Ok(decrypted_ip.to_string().into())
}

#[derive(Clone, Copy, Debug)]
pub struct DecryptIp;

impl Function for DecryptIp {
    fn identifier(&self) -> &'static str {
        "decrypt_ip"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "ip",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "key",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "mode",
                kind: kind::BYTES,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "Decrypt IPv4 with AES128 mode",
                source: r#"decrypt_ip!("72b9:a747:f2e9:72af:76ca:5866:6dcf:c3b0", "sixteen byte key", "aes128")"#,
                result: Ok("192.168.1.1"),
            },
            Example {
                title: "Decrypt IPv6 with PFX mode",
                source: r#"decrypt_ip!("88bd:d2bf:8865:8c4d:84b:44f6:6077:72c9", "thirty-two bytes key for ipv6pfx", "pfx")"#,
                result: Ok("2001:db8::1"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let ip = arguments.required("ip");
        let key = arguments.required("key");
        let mode = arguments.required("mode");

        Ok(DecryptIpFn { ip, key, mode }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct DecryptIpFn {
    ip: Box<dyn Expression>,
    key: Box<dyn Expression>,
    mode: Box<dyn Expression>,
}

impl FunctionExpression for DecryptIpFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let ip = self.ip.resolve(ctx)?;
        let key = self.key.resolve(ctx)?;
        let mode = self.mode.resolve(ctx)?;
        decrypt_ip(&ip, key, &mode)
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        decrypt_ip => DecryptIp;

        ipv4_aes128 {
            args: func_args![
                ip: "a6d8:a149:6bcf:b175:bad6:3e56:d72d:4fdb",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"),
                mode: "aes128"
            ],
            want: Ok(value!("192.168.1.1")),
            tdef: TypeDef::bytes().fallible(),
        }

        ipv4_pfx {
            args: func_args![
                ip: "194.20.195.96",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f\x20"),
                mode: "pfx"
            ],
            want: Ok(value!("192.168.1.1")),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_mode {
            args: func_args![
                ip: "192.168.1.1",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"),
                mode: "invalid"
            ],
            want: Err("Invalid mode 'invalid'. Must be 'aes128' or 'pfx'"),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_ip {
            args: func_args![
                ip: "not an ip",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"),
                mode: "aes128"
            ],
            want: Err("unable to parse IP address: invalid IP address syntax"),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_key_size_ipv4_aes128 {
            args: func_args![
                ip: "192.168.1.1",
                key: value!(b"short"),
                mode: "aes128"
            ],
            want: Err("aes128 mode requires a 16-byte key for IPv4"),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_key_size_ipv4_pfx {
            args: func_args![
                ip: "192.168.1.1",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"),
                mode: "pfx"
            ],
            want: Err("pfx mode requires a 32-byte key for IPv4"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
