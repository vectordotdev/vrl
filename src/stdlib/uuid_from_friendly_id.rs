use crate::compiler::prelude::*;
use bytes::Bytes;

fn uuid_from_friendly_id(value: &Value) -> Resolved {
    let mut buf = [0; 36];
    let value = value.try_bytes_utf8_lossy()?;
    match base62::decode(value.as_ref()) {
        Err(err) => Err(format!("failed to decode friendly id: {err}").into()),
        Ok(w128) => {
            let uuid = uuid::Uuid::from_u128(w128)
                .hyphenated()
                .encode_lower(&mut buf);
            Ok(Bytes::copy_from_slice(uuid.as_bytes()).into())
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct UuidFromFriendlyId;

impl Function for UuidFromFriendlyId {
    fn identifier(&self) -> &'static str {
        "uuid_from_friendly_id"
    }

    fn usage(&self) -> &'static str {
        "Convert a Friendly ID (base62 encoding a 128-bit word) to a UUID."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` is a string but the text uses characters outside of class [0-9A-Za-z].",
            "`value` is a base62 encoding of an integer, but the integer is greater than or equal to 2^128.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "A string that is a Friendly ID",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Convert a Friendly ID to a UUID",
            source: r#"uuid_from_friendly_id!("3s87yEvnmkiPBMHsj8bwwc")"#,
            result: Ok("7f41deed-d5e2-8b5e-7a13-ab4ff93cfad2"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(UuidFromFriendlyIdFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UuidFromFriendlyIdFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for UuidFromFriendlyIdFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        uuid_from_friendly_id(&value)
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
        uuid_from_friendly_id => UuidFromFriendlyId;
        example_from_docs {
            args: func_args![value: value!("3s87yEvnmkiPBMHsj8bwwc")],
            want: Ok(value!("7f41deed-d5e2-8b5e-7a13-ab4ff93cfad2")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
