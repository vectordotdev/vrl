use crate::compiler::prelude::*;
use uuid::{NoContext, timestamp::Timestamp};

static DEFAULT_TIMESTAMP: Value = Value::from_static_str("`now()`");

const PARAMETERS: &[Parameter] = &[Parameter::optional(
    "timestamp",
    kind::TIMESTAMP,
    "The timestamp used to generate the UUIDv7.",
)
.default(&DEFAULT_TIMESTAMP)];

#[allow(clippy::cast_sign_loss)] // TODO consider removal options
fn uuid_v7(timestamp: Option<Value>) -> Resolved {
    let utc_timestamp = if let Some(timestamp) = timestamp {
        timestamp.try_timestamp()?
    } else {
        jiff::Timestamp::now()
    };

    let nanos = i64::try_from(utc_timestamp.as_nanosecond())
        .map_err(|_| ValueError::OutOfRange(Kind::timestamp()))?;
    let seconds = nanos.div_euclid(1_000_000_000) as u64;
    let nanoseconds =
        u32::try_from(nanos.rem_euclid(1_000_000_000)).expect("sub-second nanoseconds fit in u32");
    let timestamp = Timestamp::from_unix(NoContext, seconds, nanoseconds);

    let mut buffer = [0; 36];
    let uuid = uuid::Uuid::new_v7(timestamp)
        .hyphenated()
        .encode_lower(&mut buffer);
    Ok(Value::from(&*uuid))
}

#[derive(Clone, Copy, Debug)]
pub struct UuidV7;

impl Function for UuidV7 {
    fn identifier(&self) -> &'static str {
        "uuid_v7"
    }

    fn description(&self) -> &'static str {
        "Generates a random [UUIDv7](https://datatracker.ietf.org/doc/html/draft-peabody-dispatch-new-uuid-format-04#name-uuid-version-7) string."
    }

    fn category(&self) -> &'static str {
        Category::Random.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Create a UUIDv7 with implicit `now()`",
                source: "uuid_v7()",
                result: Ok("0135ddb4-a444-794c-a7a2-088f260104c0"),
                deterministic: false,
            },
            example! {
                title: "Create a UUIDv7 with explicit `now()`",
                source: "uuid_v7(now())",
                result: Ok("0135ddb4-a444-794c-a7a2-088f260104c0"),
                deterministic: false,
            },
            example! {
                title: "Create a UUIDv7 with custom timestamp",
                source: "uuid_v7(t'2020-12-30T22:20:53.824727Z')",
                result: Ok("0176b5bd-58c0-794c-a7a2-088f260104c0"),
                deterministic: false,
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
    use crate::value::ObjectMap;
    use crate::value::Value;

    test_type_def![default {
        expr: |_| { UuidV7Fn { timestamp: None } },
        want: TypeDef::bytes().infallible(),
    }];

    #[test]
    fn uuid_v7() {
        let mut state = state::RuntimeState::default();
        let mut object: Value = Value::Object(ObjectMap::new());
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut object, &mut state, &tz);
        let value = UuidV7Fn { timestamp: None }.resolve(&mut ctx).unwrap();

        uuid::Uuid::parse_str(value.as_string().expect("UUIDv7 must be a string"))
            .expect("valid UUID V7");
    }

    #[test]
    fn uuid_v7_preserves_millisecond_timestamp() {
        for input in [
            "1970-01-01T00:00:00Z",
            "2020-12-30T22:20:53.824727Z",
            "2026-01-01T00:00:00.123456789Z",
            "2026-01-01T00:00:00.999999999Z",
        ] {
            let timestamp: jiff::Timestamp = input.parse().unwrap();
            let expected = (
                u64::try_from(timestamp.as_second()).unwrap(),
                u32::try_from(timestamp.subsec_millisecond()).unwrap() * 1_000_000,
            );

            let encoded = super::uuid_v7(Some(timestamp.into())).unwrap();
            let encoded = encoded.as_string().expect("UUIDv7 must be a string");
            let id = uuid::Uuid::parse_str(encoded).unwrap();
            assert_eq!(id.get_timestamp().unwrap().to_unix(), expected, "{input}");
        }
    }
}
