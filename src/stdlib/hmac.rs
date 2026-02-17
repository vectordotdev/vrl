use crate::compiler::function::EnumVariant;
use crate::compiler::prelude::*;
use hmac::{Hmac as HmacHasher, Mac};
use sha_2::{Sha224, Sha256, Sha384, Sha512};
use sha1::Sha1;
use std::sync::LazyLock;

macro_rules! hmac {
    ($algorithm:ty, $key:expr_2021, $val:expr_2021) => {{
        let mut mac =
            <HmacHasher<$algorithm>>::new_from_slice($key.as_ref()).expect("key is bytes");
        mac.update($val.as_ref());
        let result = mac.finalize();
        let code_bytes = result.into_bytes();
        code_bytes.to_vec()
    }};
}

static DEFAULT_ALGORITHM: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("SHA-256")));

static ALGORITHM_ENUM: &[EnumVariant] = &[
    EnumVariant {
        value: "SHA1",
        description: "SHA1 algorithm",
    },
    EnumVariant {
        value: "SHA-224",
        description: "SHA-224 algorithm",
    },
    EnumVariant {
        value: "SHA-256",
        description: "SHA-256 algorithm",
    },
    EnumVariant {
        value: "SHA-384",
        description: "SHA-384 algorithm",
    },
    EnumVariant {
        value: "SHA-512",
        description: "SHA-512 algorithm",
    },
];

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to calculate the HMAC for.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "key",
            kind: kind::BYTES,
            required: true,
            description: "The string to use as the cryptographic key.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "algorithm",
            kind: kind::BYTES,
            required: false,
            description: "The hashing algorithm to use.",
            default: Some(&DEFAULT_ALGORITHM),
            enum_variants: Some(ALGORITHM_ENUM),
        },
    ]
});

fn hmac(value: Value, key: Value, algorithm: &Value) -> Resolved {
    let value = value.try_bytes()?;
    let key = key.try_bytes()?;
    let algorithm = algorithm.try_bytes_utf8_lossy()?.as_ref().to_uppercase();

    let code_bytes = match algorithm.as_str() {
        "SHA1" => hmac!(Sha1, key, value),
        "SHA-224" => hmac!(Sha224, key, value),
        "SHA-256" => hmac!(Sha256, key, value),
        "SHA-384" => hmac!(Sha384, key, value),
        "SHA-512" => hmac!(Sha512, key, value),
        _ => return Err(format!("Invalid algorithm: {algorithm}").into()),
    };

    Ok(Value::Bytes(Bytes::from(code_bytes)))
}

#[derive(Clone, Copy, Debug)]
pub struct Hmac;

impl Function for Hmac {
    fn identifier(&self) -> &'static str {
        "hmac"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Calculates a [HMAC](https://en.wikipedia.org/wiki/HMAC) of the `value` using the given `key`.
            The hashing `algorithm` used can be optionally specified.

            For most use cases, the resulting bytestream should be encoded into a hex or base64
            string using either [encode_base16](/docs/reference/vrl/functions/#encode_base16) or
            [encode_base64](/docs/reference/vrl/functions/#encode_base64).

            This function is infallible if either the default `algorithm` value or a recognized-valid compile-time
            `algorithm` string literal is used. Otherwise, it is fallible.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Cryptography.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Calculate message HMAC (defaults: SHA-256), encoding to a base64 string",
                source: r#"encode_base64(hmac("Hello there", "super-secret-key"))"#,
                result: Ok("eLGE8YMviv85NPXgISRUZxstBNSU47JQdcXkUWcClmI="),
            },
            example! {
                title: "Calculate message HMAC using SHA-224, encoding to a hex-encoded string",
                source: r#"encode_base16(hmac("Hello there", "super-secret-key", algorithm: "SHA-224"))"#,
                result: Ok("42fccbc2b7d22a143b92f265a8046187558a94d11ddbb30622207e90"),
            },
            example! {
                title: "Calculate message HMAC using SHA1, encoding to a base64 string",
                source: r#"encode_base64(hmac("Hello there", "super-secret-key", algorithm: "SHA1"))"#,
                result: Ok("MiyBIHO8Set9+6crALiwkS0yFPE="),
            },
            example! {
                title: "Calculate message HMAC using a variable hash algorithm",
                source: r#"
.hash_algo = "SHA-256"
hmac_bytes, err = hmac("Hello there", "super-secret-key", algorithm: .hash_algo)
if err == null {
	.hmac = encode_base16(hmac_bytes)
}
"#,
                result: Ok("78b184f1832f8aff3934f5e0212454671b2d04d494e3b25075c5e45167029662"),
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
        let key = arguments.required("key");
        let algorithm = arguments.optional("algorithm");

        Ok(HmacFn {
            value,
            key,
            algorithm,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct HmacFn {
    value: Box<dyn Expression>,
    key: Box<dyn Expression>,
    algorithm: Option<Box<dyn Expression>>,
}

impl FunctionExpression for HmacFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let key = self.key.resolve(ctx)?;
        let algorithm = self
            .algorithm
            .map_resolve_with_default(ctx, || DEFAULT_ALGORITHM.clone())?;

        hmac(value, key, &algorithm)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let valid_algorithms = ["SHA1", "SHA-224", "SHA-256", "SHA-384", "SHA-512"];

        let mut valid_static_algo = false;
        if let Some(algorithm) = self.algorithm.as_ref() {
            if let Some(algorithm) = algorithm.resolve_constant(state)
                && let Ok(algorithm) = algorithm.try_bytes_utf8_lossy()
            {
                let algorithm = algorithm.to_uppercase();
                valid_static_algo = valid_algorithms.contains(&algorithm.as_str());
            }
        } else {
            valid_static_algo = true;
        }

        if valid_static_algo {
            TypeDef::bytes().infallible()
        } else {
            TypeDef::bytes().fallible()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        hmac => Hmac;

        hmac {
            args: func_args![key: "super-secret-key", value: "Hello there"],
            want: Ok(value!(b"x\xb1\x84\xf1\x83/\x8a\xff94\xf5\xe0!$Tg\x1b-\x04\xd4\x94\xe3\xb2Pu\xc5\xe4Qg\x02\x96b")),
            tdef: TypeDef::bytes().infallible(),
        }

        hmac_sha1 {
            args: func_args![key: "super-secret-key", value: "Hello there", algorithm: "SHA1"],
            want: Ok(value!(b"2,\x81 s\xbcI\xeb}\xfb\xa7+\x00\xb8\xb0\x91-2\x14\xf1")),
            tdef: TypeDef::bytes().infallible(),
        }

        hmac_sha224 {
            args: func_args![key: "super-secret-key", value: "Hello there", algorithm: "SHA-224"],
            want: Ok(value!(b"B\xfc\xcb\xc2\xb7\xd2*\x14;\x92\xf2e\xa8\x04a\x87U\x8a\x94\xd1\x1d\xdb\xb3\x06\" ~\x90")),
            tdef: TypeDef::bytes().infallible(),
        }

        hmac_sha384 {
            args: func_args![key: "super-secret-key", value: "Hello there", algorithm: "SHA-384"],
            want: Ok(value!(b"\xe2Q7\xc4\xd7\xde\xa2\xcc\xb9&#`\xf5s\x88M[\x81\x8f=\x0d\xb7\x92\x976?fB\x94\xf3\x88\xf0\xf9\xb5\x8c\x04\xc1\x1d\x88\x06\xb5`\xb8\x0d\xe0?\xed\x0d")),
            tdef: TypeDef::bytes().infallible(),
        }

        hmac_sha512 {
            args: func_args![key: "super-secret-key", value: "Hello there", algorithm: "SHA-512"],
            want: Ok(value!(b" \xc9*\x07k\"\xf3C+\xfe\x91\x8d\xfeC\x14\xd0$<\x85\x08d:\xb1\xd7\xd7y\xa5e\x84\x81\xce/\xd4\x08!\x04@\x10\xe9x\xc16Q\x7fX\xff\xc8\xe6\xc1\xf2X0s\x88X0<\xf0\xa7\x10s\xc6\x0e\x96")),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
