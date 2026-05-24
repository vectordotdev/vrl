use bytes::Bytes;
use chrono::{DateTime, Utc};
use ordered_float::NotNan;
use quickcheck::{Arbitrary, Gen};
use rust_decimal::Decimal;

use super::Value;

const MAX_ARRAY_SIZE: usize = 4;
const MAX_MAP_SIZE: usize = 4;
const MAX_F64_SIZE: f64 = 1_000_000.0;
const MAX_DECIMAL_SIZE: i64 = 1_000_000;

fn datetime(g: &mut Gen) -> DateTime<Utc> {
    // `chrono` documents that there is an out-of-range for both second and
    // nanosecond values but doesn't actually document what the valid ranges
    // are. We just sort of arbitrarily restrict things.
    let secs = i64::arbitrary(g) % 32_000;
    let nanoseconds = u32::arbitrary(g) % 32_000;
    DateTime::<Utc>::from_timestamp(secs, nanoseconds).expect("invalid timestamp")
}

// When generating fixtures we need f64 values that survive a JSON round-trip
// without any loss of precision or serialization ambiguity (NaN, -0.0).
// Under the `generate-fixtures` feature the helper produces clean values;
// otherwise it falls back to the standard quickcheck approach.
fn f64_for_arbitrary(g: &mut Gen) -> f64 {
    #[cfg(feature = "generate-fixtures")]
    {
        let mut value = f64::arbitrary(g) % MAX_F64_SIZE;
        while value.is_nan() || value == -0.0 {
            value = f64::arbitrary(g) % MAX_F64_SIZE;
        }
        let rounded = (value * 10_000.0).round() / 10_000.0;
        // Rounding can produce -0.0 from small negatives; normalize to +0.0.
        if rounded == -0.0_f64 { 0.0 } else { rounded }
    }
    #[cfg(not(feature = "generate-fixtures"))]
    {
        f64::arbitrary(g) % MAX_F64_SIZE
    }
}

impl Arbitrary for Value {
    fn arbitrary(g: &mut Gen) -> Self {
        // Quickcheck can't derive Arbitrary for enums, see
        // https://github.com/BurntSushi/quickcheck/issues/98.  The magical
        // constant here are the number of fields in `Value`. With 9 variants,
        // there's a slight bias but it's acceptable for testing.
        let choice = u8::arbitrary(g) % 9;

        // Under `generate-fixtures`, Timestamp (slot 5) is excluded because it
        // doesn't survive a JSON/protobuf round-trip cleanly. Nudge it to
        // Object (slot 6) instead.
        #[cfg(feature = "generate-fixtures")]
        let choice = { if choice == 5 { 6 } else { choice } };

        match choice {
            0 => {
                #[cfg(not(feature = "generate-fixtures"))]
                let bytes: Vec<u8> = Vec::arbitrary(g);
                // Under `generate-fixtures`, use valid UTF-8 so bytes values
                // survive a JSON round-trip without encoding issues.
                #[cfg(feature = "generate-fixtures")]
                let bytes = String::arbitrary(g).into_bytes();
                Self::Bytes(Bytes::from(bytes))
            }
            1 => Self::Integer(i64::arbitrary(g)),
            2 => {
                let f = f64_for_arbitrary(g);
                let not_nan = NotNan::new(f).unwrap_or_else(|_| NotNan::new(0.0).unwrap());
                Self::from(not_nan)
            }
            3 => {
                let d = i64::arbitrary(g) % MAX_DECIMAL_SIZE;
                Self::Decimal(Decimal::new(d, 2))
            }
            4 => Self::Boolean(bool::arbitrary(g)),
            5 => Self::Timestamp(datetime(g)),
            6 => {
                #[cfg(feature = "generate-fixtures")]
                let mut generator = Gen::from_size_and_seed(MAX_MAP_SIZE, u64::arbitrary(g));
                #[cfg(not(feature = "generate-fixtures"))]
                let mut generator = Gen::new(MAX_MAP_SIZE);
                Self::Object(
                    // `Arbitrary` is not directly implemented for `KeyString` so have to convert.
                    Vec::<(String, Self)>::arbitrary(&mut generator)
                        .into_iter()
                        .map(|(k, v)| (k.into(), v))
                        .collect(),
                )
            }
            7 => {
                #[cfg(feature = "generate-fixtures")]
                let mut generator = Gen::from_size_and_seed(MAX_ARRAY_SIZE, u64::arbitrary(g));
                #[cfg(not(feature = "generate-fixtures"))]
                let mut generator = Gen::new(MAX_ARRAY_SIZE);
                Self::Array(Vec::arbitrary(&mut generator))
            }
            8 => Self::Null,
            _ => unreachable!(),
        }
    }
}
