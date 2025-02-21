use crate::compiler::prelude::*;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use uuid::{timestamp::Timestamp, NoContext};

#[allow(clippy::cast_sign_loss)] // TODO consider removal options
fn uuid_v7(timestamp: Option<Value>) -> Resolved {
    let utc_timestamp: DateTime<Utc> = if let Some(timestamp) = timestamp {
        timestamp.try_timestamp()?
    } else {
        Utc::now()
    };

    let seconds = utc_timestamp.timestamp() as u64;
    let nanoseconds = match utc_timestamp.timestamp_nanos_opt() {
        #[allow(clippy::cast_possible_truncation)] //TODO evaluate removal options
        Some(nanos) => nanos as u32,
        None => return Err(ValueError::OutOfRange(Kind::timestamp()).into()),
    };
    let timestamp = Timestamp::from_unix(NoContext, seconds, nanoseconds);

    let mut buffer = [0; 36];
    let uuid = uuid::Uuid::new_v7(timestamp)
        .hyphenated()
        .encode_lower(&mut buffer);
    Ok(Bytes::copy_from_slice(uuid.as_bytes()).into())
}

#[derive(Clone, Copy, Debug)]
pub struct UuidV7;

impl Function for UuidV7 {
    fn identifier(&self) -> &'static str {
        "uuid_v7"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "timestamp",
            kind: kind::TIMESTAMP,
            required: false,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid with implicit now()",
                source: r#"uuid_v7() != """#,
                result: Ok("true"),
            },
            Example {
                title: "valid with explicit now()",
                source: r#"uuid_v7(now()) != """#,
                result: Ok("true"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let timestamp = arguments.optional("timestamp");

        Ok(UuidV7Fn { timestamp }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UuidV7Fn {
    timestamp: Option<Box<dyn Expression>>,
}

impl FunctionExpression for UuidV7Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let timestamp = self
            .timestamp
            .as_ref()
            .map(|m| m.resolve(ctx))
            .transpose()?;

        uuid_v7(timestamp)
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
        expr: |_| { UuidV7Fn { timestamp: None } },
        want: TypeDef::bytes().infallible(),
    }];

    #[test]
    fn uuid_v7() {
        let mut state = state::RuntimeState::default();
        let mut object: Value = Value::Object(BTreeMap::new());
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut object, &mut state, &tz);
        let value = UuidV7Fn { timestamp: None }.resolve(&mut ctx).unwrap();

        assert!(matches!(&value, Value::Bytes(_)));

        match value {
            Value::Bytes(val) => {
                let val = String::from_utf8_lossy(&val);
                uuid::Uuid::parse_str(&val).expect("valid UUID V7");
            }
            _ => unreachable!(),
        }
    }
}
