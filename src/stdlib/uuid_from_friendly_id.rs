use crate::compiler::prelude::*;
use crate::stdlib::string_utils::convert_to_string;
use bytes::Bytes;

fn uuid_from_friendly_id(value: Value) -> Resolved {
    let mut buf = [0; 36];
    let value = convert_to_string(value, false)?;
    match base62::decode(value) {
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

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "Decode UUID from 128-bit Friendly ID",
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
        uuid_from_friendly_id(value)
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
