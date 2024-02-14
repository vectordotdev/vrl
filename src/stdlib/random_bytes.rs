use crate::compiler::prelude::*;
use rand::{thread_rng, RngCore};

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

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "length",
            kind: kind::INTEGER,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "get 16 random bytes",
            source: "length(random_bytes(16))",
            result: Ok("16"),
        }]
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
    Ok(length as usize)
}

#[derive(Debug, Clone)]
struct RandomBytesFn {
    length: Box<dyn Expression>,
}

impl FunctionExpression for RandomBytesFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let length = self.length.resolve(ctx)?;
        random_bytes(length)
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
