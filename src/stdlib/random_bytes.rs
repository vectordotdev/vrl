use crate::compiler::prelude::*;
use rand::{RngCore, thread_rng};

const MAX_LENGTH: i64 = 1024 * 64;
const LENGTH_TOO_LARGE_ERR: &str = "Length is too large. Maximum is 64k";
const LENGTH_TOO_SMALL_ERR: &str = "Length cannot be negative";

fn random_bytes(length: Value) -> Resolved {
    let mut output = vec![0_u8; get_length(length)?];

    // ThreadRng is a cryptographically secure generator
    thread_rng().fill_bytes(&mut output);

    Ok(Value::Bytes(Bytes::from(output)))
}

#[derive(Clone, Copy, Debug)]
pub struct RandomBytes;

impl Function for RandomBytes {
    fn identifier(&self) -> &'static str {
        "random_bytes"
    }

    fn usage(&self) -> &'static str {
        "A cryptographically secure random number generator. Returns a string value containing the number of random bytes requested."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "length",
            kind: kind::INTEGER,
            required: true,
            description: "The number of bytes to generate. Must not be larger than 64k.",
        }]
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Generate 16 random bytes",
            source: "length(random_bytes(16))",
            result: Ok("16"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Generate random base 64 encoded bytes",
                source: "encode_base64(random_bytes(16))",
                result: Ok("LNu0BBgUbh7XAlXbjSOomQ=="),
            },
            example! {
                title: "Generate 16 random bytes",
                source: "length(random_bytes(16))",
                result: Ok("16"),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let length = arguments.required("length");

        if let Some(literal) = length.resolve_constant(state) {
            // check if length is valid
            let _: usize =
                get_length(literal.clone()).map_err(|err| function::Error::InvalidArgument {
                    keyword: "length",
                    value: literal,
                    error: err,
                })?;
        }

        Ok(RandomBytesFn { length }.as_expr())
    }
}

fn get_length(value: Value) -> std::result::Result<usize, &'static str> {
    let length = value.try_integer().expect("length must be an integer");
    if length < 0 {
        return Err(LENGTH_TOO_SMALL_ERR);
    }
    if length > MAX_LENGTH {
        return Err(LENGTH_TOO_LARGE_ERR);
    }
    // TODO consider removal options
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    Ok(length as usize)
}

#[derive(Debug, Clone)]
struct RandomBytesFn {
    length: Box<dyn Expression>,
}

impl FunctionExpression for RandomBytesFn {
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let length = self.length.resolve(ctx)?;
        random_bytes(length)
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let length = self.length.resolve(ctx)?;
        if length.try_into_i64().unwrap() == 16 {
            let fixed_bytes = vec![
                0x2C, 0xDB, 0xB4, 0x04, 0x18, 0x14, 0x6E, 0x1E, 0xD7, 0x02, 0x55, 0xDB, 0x8D, 0x23,
                0xA8, 0x99,
            ];
            Ok(Value::Bytes(Bytes::from(fixed_bytes)))
        } else {
            random_bytes(length)
        }
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        match self.length.resolve_constant(state) {
            None => TypeDef::bytes().fallible(),
            Some(value) => {
                if get_length(value).is_ok() {
                    TypeDef::bytes()
                } else {
                    TypeDef::bytes().fallible()
                }
            }
        }
    }
}
