use crate::compiler::prelude::*;

fn uuid_v4() -> Value {
    let mut buf = [0; 36];
    let uuid = uuid::Uuid::new_v4().hyphenated().encode_lower(&mut buf);
    Value::from(&*uuid)
}

#[derive(Clone, Copy, Debug)]
pub struct UuidV4;

impl Function for UuidV4 {
    fn identifier(&self) -> &'static str {
        "uuid_v4"
    }

    fn usage(&self) -> &'static str {
        "Generates a random [UUIDv4](https://en.wikipedia.org/wiki/Universally_unique_identifier#Version_4_(random)) string."
    }

    fn category(&self) -> &'static str {
        Category::Random.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Create a UUIDv4",
            source: "uuid_v4()",
            result: Ok("1d262f4f-199b-458d-879f-05fd0a5f0683"),
            deterministic: false,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(UuidV4Fn.as_expr())
    }
}

#[derive(Debug, Clone, Copy)]
struct UuidV4Fn;

impl FunctionExpression for UuidV4Fn {
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(uuid_v4())
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use std::collections::BTreeMap;

    test_type_def![default {
        expr: |_| { UuidV4Fn },
        want: TypeDef::bytes().infallible(),
    }];

    #[test]
    fn uuid_v4() {
        let mut state = state::RuntimeState::default();
        let mut object: Value = Value::Object(BTreeMap::new());
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut object, &mut state, &tz);
        let value = UuidV4Fn.resolve(&mut ctx).unwrap();

        let text = value.as_string().expect("UUIDv4 must be a string");
        uuid::Uuid::parse_str(text).expect("valid UUID V4");
    }
}
