use crate::compiler::prelude::*;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::sync::LazyLock;
use uuid::{NoContext, timestamp::Timestamp};

static DEFAULT_TIMESTAMP: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("`now()`")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![Parameter {
        keyword: "timestamp",
        kind: kind::TIMESTAMP,
        required: false,
        description: "The timestamp used to generate the UUIDv7.",
        default: Some(&DEFAULT_TIMESTAMP),
        enum_variants: None,
    }]
});

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

    fn usage(&self) -> &'static str {
        "Generates a random [UUIDv7](https://datatracker.ietf.org/doc/html/draft-peabody-dispatch-new-uuid-format-04#name-uuid-version-7) string."
    }

    fn category(&self) -> &'static str {
        Category::Random.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Create a UUIDv7 with implicit `now()`",
                source: r#"uuid_v7() != """#,
                result: Ok("true"),
            },
            example! {
                title: "Create a UUIDv7 with explicit `now()`",
                source: r#"uuid_v7(now()) != """#,
                result: Ok("true"),
            },
            example! {
                title: "Create a UUIDv7 with custom timestamp",
                source: r#"uuid_v7(t'2020-12-30T22:20:53.824727Z') != """#,
                result: Ok("true"),
            },
        ]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Create a UUIDv7 with implicit `now()`",
                source: r#"uuid_v7()"#,
                result: Ok("0135ddb4-a444-794c-a7a2-088f260104c0"),
            },
            example! {
                title: "Create a UUIDv7 with explicit `now()`",
                source: r#"uuid_v7(now())"#,
                result: Ok("0135ddb4-a444-794c-a7a2-088f260104c0"),
            },
            example! {
                title: "Create a UUIDv7 with custom timestamp",
                source: r#"uuid_v7(t'2020-12-30T22:20:53.824727Z')"#,
                result: Ok("0176b5bd-5d19-794c-a7a2-088f260104c0"),
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

        // Use mocked now() implementation if timestamp is missing
        #[cfg(feature = "__mock_return_values_for_tests")]
        let timestamp =
            timestamp.or_else(|| super::Now {}.compile(_state, _ctx, Default::default()).ok());

        Ok(UuidV7Fn { timestamp }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UuidV7Fn {
    timestamp: Option<Box<dyn Expression>>,
}

impl FunctionExpression for UuidV7Fn {
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let timestamp = self
            .timestamp
            .as_ref()
            .map(|m| m.resolve(ctx))
            .transpose()?;

        uuid_v7(timestamp)
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let timestamp = self
            .timestamp
            .as_ref()
            .map(|m| m.resolve(ctx))
            .transpose()?;

        let uuid = uuid_v7(timestamp)?;

        let crate::value::Value::Bytes(uuid) = uuid else {
            unreachable!()
        };

        Ok(crate::value::Value::Bytes(
            format!(
                "{}94c-a7a2-088f260104c0",
                str::from_utf8(&uuid[..15]).unwrap(),
            )
            .into(),
        ))
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
