use crate::compiler::prelude::*;
use bytes::Bytes;

#[cfg_attr(feature = "__mock_return_values_for_tests", allow(dead_code))]
fn uuid_v4() -> Value {
    let mut buf = [0; 36];
    let uuid = uuid::Uuid::new_v4().hyphenated().encode_lower(&mut buf);
    Bytes::copy_from_slice(uuid.as_bytes()).into()
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

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Create a UUIDv4",
            source: r#"uuid_v4() != """#,
            result: Ok("true"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Create a UUIDv4",
            source: r#"uuid_v4()"#,
            result: Ok("1d262f4f-199b-458d-879f-05fd0a5f0683"),
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
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(uuid_v4())
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok("1d262f4f-199b-458d-879f-05fd0a5f0683".into())
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

        assert!(matches!(&value, Value::Bytes(_)));

        match value {
            Value::Bytes(val) => {
                let val = String::from_utf8_lossy(&val);
                uuid::Uuid::parse_str(&val).expect("valid UUID V4");
            }
            _ => unreachable!(),
        }
    }
}
