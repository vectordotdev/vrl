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

    fn usage(&self) -> &'static str {
        indoc! {"
            Decrypts an IP address that was previously encrypted, restoring the original IP address.

            Supported Modes:

            * AES128 - Decrypts an IP address that was scrambled using AES-128 encryption. Can transform between IPv4 and IPv6.
            * PFX (Prefix-preserving) - Decrypts an IP address that was encrypted with prefix-preserving mode, where network hierarchy was maintained.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "ip",
                kind: kind::BYTES,
                required: true,
                description: "The encrypted IP address to decrypt (v4 or v6).",
            },
            Parameter {
                keyword: "key",
                kind: kind::BYTES,
                required: true,
                description: "The decryption key in raw bytes (not encoded). Must be the same key that was used for encryption. For AES128 mode, the key must be exactly 16 bytes. For PFX mode, the key must be exactly 32 bytes.",
            },
            Parameter {
                keyword: "mode",
                kind: kind::BYTES,
                required: true,
                description: "The decryption mode to use. Must match the mode used for encryption: either `aes128` or `pfx`.",
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Decrypt IPv4 address with AES128",
                source: r#"decrypt_ip!("72b9:a747:f2e9:72af:76ca:5866:6dcf:c3b0", "sixteen byte key", "aes128")"#,
                result: Ok("192.168.1.1"),
            },
            example! {
                title: "Decrypt IPv6 address with AES128",
                source: r#"decrypt_ip!("c0e6:eb35:6887:f554:4c65:8ace:17ca:6c6a", "sixteen byte key", "aes128")"#,
                result: Ok("2001:db8::1"),
            },
            example! {
                title: "Decrypt IPv4 address with prefix-preserving mode",
                source: r#"decrypt_ip!("33.245.248.61", "thirty-two bytes key for pfx use", "pfx")"#,
                result: Ok("192.168.1.1"),
            },
            example! {
                title: "Decrypt IPv6 address with prefix-preserving mode",
                source: r#"decrypt_ip!("88bd:d2bf:8865:8c4d:84b:44f6:6077:72c9", "thirty-two bytes key for ipv6pfx", "pfx")"#,
                result: Ok("2001:db8::1"),
            },
            example! {
                title: "Round-trip encryption and decryption",
                source: r#"decrypt_ip!(encrypt_ip!("192.168.1.100", "sixteen byte key", "aes128"), "sixteen byte key", "aes128")"#,
                result: Ok("192.168.1.100"),
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
