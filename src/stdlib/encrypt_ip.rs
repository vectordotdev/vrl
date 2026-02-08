use crate::compiler::prelude::*;
use crate::stdlib::ip_utils::to_key;
use ipcrypt_rs::{Ipcrypt, IpcryptPfx};
use std::net::IpAddr;

fn encrypt_ip(ip: &Value, key: Value, mode: &Value) -> Resolved {
    let ip_str = ip.try_bytes_utf8_lossy()?;
    let ip_addr: IpAddr = ip_str
        .parse()
        .map_err(|err| format!("unable to parse IP address: {err}"))?;

    let mode_str = mode.try_bytes_utf8_lossy()?;

    let ip_ver_label = match ip_addr {
        IpAddr::V4(_) => "IPv4",
        IpAddr::V6(_) => "IPv6",
    };

    let encrypted_ip = match mode_str.as_ref() {
        "aes128" => {
            let key = to_key::<16>(key, "aes128", ip_ver_label)?;
            Ipcrypt::new(key).encrypt_ipaddr(ip_addr)
        }
        "pfx" => {
            let key = to_key::<32>(key, "pfx", ip_ver_label)?;
            IpcryptPfx::new(key).encrypt_ipaddr(ip_addr)
        }
        other => {
            return Err(format!("Invalid mode '{other}'. Must be 'aes128' or 'pfx'").into());
        }
    };

    Ok(encrypted_ip.to_string().into())
}

#[derive(Clone, Copy, Debug)]
pub struct EncryptIp;

impl Function for EncryptIp {
    fn identifier(&self) -> &'static str {
        "encrypt_ip"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Encrypts an IP address, transforming it into a different valid IP address.

            Supported Modes:

            * AES128 - Scrambles the entire IP address using AES-128 encryption. Can transform between IPv4 and IPv6.
            * PFX (Prefix-preserving) - Maintains network hierarchy by ensuring that IP addresses within the same network are encrypted to addresses that also share a common network. This preserves prefix relationships while providing confidentiality.
        "}
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`ip` is not a valid IP address.",
            "`mode` is not a supported mode (must be `aes128` or `pfx`).",
            "`key` length does not match the requirements for the specified mode (16 bytes for `aes128`, 32 bytes for `pfx`).",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn notices(&self) -> &'static [&'static str] {
        &[indoc! {"
            The `aes128` mode implements the `ipcrypt-deterministic` algorithm from the IPCrypt
            specification, while the `pfx` mode implements the `ipcrypt-pfx` algorithm. Both modes
            provide deterministic encryption where the same input IP address encrypted with the
            same key will always produce the same encrypted output.
        "}]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "ip",
                kind: kind::BYTES,
                required: true,
                description: "The IP address to encrypt (v4 or v6).",
                default: None,
            },
            Parameter {
                keyword: "key",
                kind: kind::BYTES,
                required: true,
                description: "The encryption key in raw bytes (not encoded). For AES128 mode, the key must be exactly 16 bytes. For PFX mode, the key must be exactly 32 bytes.",
                default: None,
            },
            Parameter {
                keyword: "mode",
                kind: kind::BYTES,
                required: true,
                description: "The encryption mode to use. Must be either `aes128` or `pfx`.",
                default: None,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Encrypt IPv4 address with AES128",
                source: r#"encrypt_ip!("192.168.1.1", "sixteen byte key", "aes128")"#,
                result: Ok("72b9:a747:f2e9:72af:76ca:5866:6dcf:c3b0"),
            },
            example! {
                title: "Encrypt IPv6 address with AES128",
                source: r#"encrypt_ip!("2001:db8::1", "sixteen byte key", "aes128")"#,
                result: Ok("c0e6:eb35:6887:f554:4c65:8ace:17ca:6c6a"),
            },
            example! {
                title: "Encrypt IPv4 address with prefix-preserving mode",
                source: r#"encrypt_ip!("192.168.1.1", "thirty-two bytes key for pfx use", "pfx")"#,
                result: Ok("33.245.248.61"),
            },
            example! {
                title: "Encrypt IPv6 address with prefix-preserving mode",
                source: r#"encrypt_ip!("2001:db8::1", "thirty-two bytes key for ipv6pfx", "pfx")"#,
                result: Ok("88bd:d2bf:8865:8c4d:84b:44f6:6077:72c9"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let ip = arguments.required("ip");
        let key = arguments.required("key");
        let mode = arguments.required("mode");

        Ok(EncryptIpFn { ip, key, mode }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct EncryptIpFn {
    ip: Box<dyn Expression>,
    key: Box<dyn Expression>,
    mode: Box<dyn Expression>,
}

impl FunctionExpression for EncryptIpFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let ip = self.ip.resolve(ctx)?;
        let key = self.key.resolve(ctx)?;
        let mode = self.mode.resolve(ctx)?;
        encrypt_ip(&ip, key, &mode)
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
        encrypt_ip => EncryptIp;

        ipv4_aes128 {
            args: func_args![
                ip: "192.168.1.1",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"),
                mode: "aes128"
            ],
            want: Ok(value!("a6d8:a149:6bcf:b175:bad6:3e56:d72d:4fdb")),
            tdef: TypeDef::bytes().fallible(),
        }

        ipv4_pfx {
            args: func_args![
                ip: "192.168.1.1",
                key: value!(b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f\x20"),
                mode: "pfx"
            ],
            want: Ok(value!("194.20.195.96")),
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
