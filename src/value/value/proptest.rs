use bytes::Bytes;
use chrono::{DateTime, Utc};
use ordered_float::NotNan;
use proptest::prelude::*;

use super::Value;
use crate::value::KeyString;

const SIZE: u32 = 4;
const MAX_F64_SIZE: f64 = 1_000_000.0;

fn datetime_strategy() -> impl Strategy<Value = DateTime<Utc>> {
    // `chrono` documents that there is an out-of-range for both second and
    // nanosecond values but doesn't actually document what the valid ranges
    // are. We just sort of arbitrarily restrict things.
    (0i64..32_000i64, 0u32..32_000u32).prop_map(|(secs, nanoseconds)| {
        DateTime::<Utc>::from_timestamp(secs, nanoseconds).expect("invalid timestamp")
    })
}

fn float_strategy() -> impl Strategy<Value = Value> {
    // proptest::num::f64::NORMAL excludes NaN and Inf, so the
    // unwrap_or is just a safety net for the modulo edge case.
    proptest::num::f64::NORMAL
        .prop_map(|f| f % MAX_F64_SIZE)
        .prop_map(|f| Value::from(NotNan::new(f).unwrap_or_else(|_| NotNan::new(0.0).unwrap())))
}

// The non-recursive leaf variants of Value. prop_recursive uses this as the
// base case when the maximum nesting depth is reached.
fn value_leaf_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Boolean),
        any::<i64>().prop_map(Value::Integer),
        float_strategy(),
        proptest::collection::vec(any::<u8>(), 0..=16).prop_map(|b| Value::Bytes(Bytes::from(b))),
        datetime_strategy().prop_map(Value::Timestamp),
    ]
}

impl proptest::arbitrary::Arbitrary for Value {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        value_leaf_strategy()
            .prop_recursive(3, 16, SIZE, |inner| {
                prop_oneof![
                    Just(Value::Null),
                    any::<bool>().prop_map(Value::Boolean),
                    any::<i64>().prop_map(Value::Integer),
                    float_strategy(),
                    proptest::collection::vec(any::<u8>(), 0..=16)
                        .prop_map(|b| Value::Bytes(Bytes::from(b))),
                    datetime_strategy().prop_map(Value::Timestamp),
                    proptest::collection::vec(
                        (any::<KeyString>(), inner.clone()),
                        0..=SIZE as usize,
                    )
                    .prop_map(|pairs| Value::Object(pairs.into_iter().collect())),
                    proptest::collection::vec(inner, 0..=SIZE as usize).prop_map(Value::Array),
                ]
            })
            .boxed()
    }
}
