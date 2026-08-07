#![deny(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_precision_loss, clippy::module_name_repetitions)]

use crate::compiler::{ExpressionError, value::VrlValueConvert};
use crate::value::{ObjectMap, Value};
use bytes::{BufMut, Bytes, BytesMut};
use ordered_float::NotNan;

use super::ValueError;

#[allow(clippy::missing_errors_doc)]
pub trait VrlValueArithmetic: Sized {
    /// Similar to [`std::ops::Mul`], but fallible (e.g. `TryMul`).
    fn try_mul(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::ops::Div`], but fallible (e.g. `TryDiv`).
    fn try_div(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::ops::Add`], but fallible (e.g. `TryAdd`).
    fn try_add(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::ops::Sub`], but fallible (e.g. `TrySub`).
    fn try_sub(self, rhs: Self) -> Result<Self, ValueError>;

    /// Try to "OR" (`||`) two values types.
    ///
    /// If the lhs value is `null` or `false`, the rhs is evaluated and
    /// returned. The rhs is a closure that can return an error, and thus this
    /// method can return an error as well.
    fn try_or(self, rhs: impl FnMut() -> Result<Self, ExpressionError>)
    -> Result<Self, ValueError>;

    /// Try to "AND" (`&&`) two values types.
    ///
    /// A lhs or rhs value of `Null` returns `false`.
    fn try_and(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::ops::Rem`], but fallible (e.g. `TryRem`).
    fn try_rem(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_gt(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_ge(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_lt(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_le(self, rhs: Self) -> Result<Self, ValueError>;

    fn try_merge(self, rhs: Self) -> Result<Self, ValueError>;

    /// Similar to [`std::cmp::Eq`], but does a lossless comparison for integers
    /// and floats.
    fn eq_lossy(&self, rhs: &Self) -> bool;
}

fn float_result(value: f64) -> Result<Value, ValueError> {
    NotNan::new(value)
        .map(Value::Float)
        .map_err(|_| ValueError::NanFloat)
}

impl VrlValueArithmetic for Value {
    /// Similar to [`std::ops::Mul`], but fallible (e.g. `TryMul`).
    fn try_mul(self, rhs: Self) -> Result<Self, ValueError> {
        // When multiplying a string by an integer, if the number is negative we set it to zero to
        // return an empty string.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let as_usize = |num| if num < 0 { 0 } else { num as usize };

        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Bytes(rhs)) => {
                Bytes::from(rhs.repeat(as_usize(lhs))).into()
            }
            (Value::Integer(lhs), Value::Float(rhs)) => {
                float_result(lhs as f64 * rhs.into_inner())?
            }
            (Value::Integer(lhs), Value::Integer(rhs)) => i64::wrapping_mul(lhs, rhs).into(),
            (Value::Float(lhs), Value::Integer(rhs)) => {
                float_result(lhs.into_inner() * rhs as f64)?
            }
            (Value::Float(lhs), Value::Float(rhs)) => {
                float_result(lhs.into_inner() * rhs.into_inner())?
            }
            (Value::Bytes(lhs), Value::Integer(rhs)) => {
                Bytes::from(lhs.repeat(as_usize(rhs))).into()
            }
            (lhs, rhs) => return Err(ValueError::Mul(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Div`], but fallible (e.g. `TryDiv`).
    fn try_div(self, rhs: Self) -> Result<Self, ValueError> {
        match (self, rhs) {
            (_, Value::Integer(0)) => Err(ValueError::DivideByZero),
            (_, Value::Float(rhs)) if rhs.into_inner() == 0.0 => Err(ValueError::DivideByZero),
            (Value::Integer(lhs), Value::Integer(rhs)) => float_result(lhs as f64 / rhs as f64),
            (Value::Integer(lhs), Value::Float(rhs)) => float_result(lhs as f64 / rhs.into_inner()),
            (Value::Float(lhs), Value::Integer(rhs)) => float_result(lhs.into_inner() / rhs as f64),
            (Value::Float(lhs), Value::Float(rhs)) => {
                float_result(lhs.into_inner() / rhs.into_inner())
            }
            (lhs, rhs) => Err(ValueError::Div(lhs.kind(), rhs.kind())),
        }
    }

    /// Similar to [`std::ops::Add`], but fallible (e.g. `TryAdd`).
    fn try_add(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => i64::wrapping_add(lhs, rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => {
                float_result(lhs as f64 + rhs.into_inner())?
            }
            (Value::Float(lhs), Value::Integer(rhs)) => {
                float_result(lhs.into_inner() + rhs as f64)?
            }
            (Value::Float(lhs), Value::Float(rhs)) => {
                float_result(lhs.into_inner() + rhs.into_inner())?
            }
            (lhs @ Value::Bytes(_), Value::Null) => lhs,
            (Value::Bytes(lhs), Value::Bytes(rhs)) => {
                #[allow(clippy::arithmetic_side_effects)]
                let mut value = BytesMut::with_capacity(lhs.len() + rhs.len());
                value.put(lhs);
                value.put(rhs);
                value.freeze().into()
            }
            (Value::Null, rhs @ Value::Bytes(_)) => rhs,
            (lhs, rhs) => return Err(ValueError::Add(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Sub`], but fallible (e.g. `TrySub`).
    fn try_sub(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => i64::wrapping_sub(lhs, rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => {
                float_result(lhs as f64 - rhs.into_inner())?
            }
            (Value::Float(lhs), Value::Integer(rhs)) => {
                float_result(lhs.into_inner() - rhs as f64)?
            }
            (Value::Float(lhs), Value::Float(rhs)) => {
                float_result(lhs.into_inner() - rhs.into_inner())?
            }
            (lhs, rhs) => return Err(ValueError::Sub(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Try to "OR" (`||`) two values types.
    ///
    /// If the lhs value is `null` or `false`, the rhs is evaluated and
    /// returned. The rhs is a closure that can return an error, and thus this
    /// method can return an error as well.
    fn try_or(
        self,
        mut rhs: impl FnMut() -> Result<Self, ExpressionError>,
    ) -> Result<Self, ValueError> {
        let err = ValueError::Or;

        match self {
            Value::Null | Value::Boolean(false) => rhs().map_err(err),
            value => Ok(value),
        }
    }

    /// Try to "AND" (`&&`) two values types.
    ///
    /// A lhs or rhs value of `Null` returns `false`.
    fn try_and(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Null, _) | (Value::Boolean(_), Value::Null) => false.into(),
            (Value::Boolean(lhs), Value::Boolean(rhs)) => (lhs && rhs).into(),
            (lhs, rhs) => return Err(ValueError::And(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Rem`], but fallible (e.g. `TryRem`).
    fn try_rem(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (_, Value::Integer(0)) => return Err(ValueError::DivideByZero),
            (_, Value::Float(rhs)) if rhs.into_inner() == 0.0 => {
                return Err(ValueError::DivideByZero);
            }
            (Value::Integer(lhs), Value::Integer(rhs)) => i64::wrapping_rem(lhs, rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => {
                float_result(lhs as f64 % rhs.into_inner())?
            }
            (Value::Float(lhs), Value::Integer(rhs)) => {
                float_result(lhs.into_inner() % rhs as f64)?
            }
            (Value::Float(lhs), Value::Float(rhs)) => {
                float_result(lhs.into_inner() % rhs.into_inner())?
            }
            (lhs, rhs) => return Err(ValueError::Rem(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_gt(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs > rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => (lhs as f64 > rhs.into_inner()).into(),
            (Value::Float(lhs), Value::Integer(rhs)) => (lhs.into_inner() > rhs as f64).into(),
            (Value::Float(lhs), Value::Float(rhs)) => (lhs > rhs).into(),
            (Value::Bytes(lhs), rhs) => (lhs > rhs.try_bytes()?).into(),
            (Value::Timestamp(lhs), rhs) => (lhs > rhs.try_timestamp()?).into(),
            (lhs, rhs) => return Err(ValueError::Rem(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_ge(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs >= rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => (lhs as f64 >= rhs.into_inner()).into(),
            (Value::Float(lhs), Value::Integer(rhs)) => (lhs.into_inner() >= rhs as f64).into(),
            (Value::Float(lhs), Value::Float(rhs)) => (lhs >= rhs).into(),
            (Value::Bytes(lhs), rhs) => (lhs >= rhs.try_bytes()?).into(),
            (Value::Timestamp(lhs), rhs) => (lhs >= rhs.try_timestamp()?).into(),
            (lhs, rhs) => return Err(ValueError::Ge(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_lt(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs < rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => ((lhs as f64) < rhs.into_inner()).into(),
            (Value::Float(lhs), Value::Integer(rhs)) => (lhs.into_inner() < rhs as f64).into(),
            (Value::Float(lhs), Value::Float(rhs)) => (lhs < rhs).into(),
            (Value::Bytes(lhs), rhs) => (lhs < rhs.try_bytes()?).into(),
            (Value::Timestamp(lhs), rhs) => (lhs < rhs.try_timestamp()?).into(),
            (lhs, rhs) => return Err(ValueError::Ge(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_le(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Integer(rhs)) => (lhs <= rhs).into(),
            (Value::Integer(lhs), Value::Float(rhs)) => (lhs as f64 <= rhs.into_inner()).into(),
            (Value::Float(lhs), Value::Integer(rhs)) => (lhs.into_inner() <= rhs as f64).into(),
            (Value::Float(lhs), Value::Float(rhs)) => (lhs <= rhs).into(),
            (Value::Bytes(lhs), rhs) => (lhs <= rhs.try_bytes()?).into(),
            (Value::Timestamp(lhs), rhs) => (lhs <= rhs.try_timestamp()?).into(),
            (lhs, rhs) => return Err(ValueError::Ge(lhs.kind(), rhs.kind())),
        };

        Ok(value)
    }

    fn try_merge(self, rhs: Self) -> Result<Self, ValueError> {
        match (self, rhs) {
            (Value::Object(lhs), Value::Object(rhs)) => {
                Ok(lhs.into_iter().chain(rhs).collect::<ObjectMap>().into())
            }
            (lhs, rhs) => Err(ValueError::Merge(lhs.kind(), rhs.kind())),
        }
    }

    /// Similar to [`std::cmp::Eq`], but does a lossless comparison for integers
    /// and floats.
    fn eq_lossy(&self, rhs: &Self) -> bool {
        use Value::{Float, Integer};

        match self {
            Integer(lhv) => rhs.try_into_f64().is_ok_and(|rhv| *lhv as f64 == rhv),

            Float(lhv) => rhs.try_into_f64().is_ok_and(|rhv| lhv.into_inner() == rhv),

            _ => self == rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float(value: f64) -> Value {
        Value::Float(NotNan::new(value).expect("test value must not be NaN"))
    }

    #[test]
    fn float_arithmetic_returns_an_error_for_nan_results() {
        assert_eq!(
            float(f64::INFINITY).try_mul(float(0.0)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(0.0).try_mul(float(f64::INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            Value::Integer(0).try_mul(float(f64::INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(f64::INFINITY).try_add(float(f64::NEG_INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(f64::NEG_INFINITY).try_add(float(f64::INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(f64::INFINITY).try_sub(float(f64::INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(f64::INFINITY).try_div(float(f64::INFINITY)),
            Err(ValueError::NanFloat)
        );
        assert_eq!(
            float(f64::INFINITY).try_rem(float(1.0)),
            Err(ValueError::NanFloat)
        );
    }

    #[test]
    fn nan_arithmetic_errors_can_be_coalesced() {
        let sources = [
            r#"(parse_float!("inf") + parse_float!("-inf")) ?? 0"#,
            r#"(parse_float!("-inf") + parse_float!("inf")) ?? 0"#,
            r#"(parse_float!("inf") * 0) ?? 0"#,
            r#"(0 * parse_float!("inf")) ?? 0"#,
            r#"(parse_float!("inf") - parse_float!("inf")) ?? 0"#,
            r#"(parse_float!("inf") / parse_float!("inf")) ?? 0"#,
            r#"mod(parse_float!("inf"), 1.0) ?? 0"#,
        ];

        for source in sources {
            let compilation = crate::compiler::compile(source, &crate::stdlib::all())
                .unwrap_or_else(|error| panic!("failed to compile `{source}`: {error:?}"));
            let mut target = Value::Object(ObjectMap::new());
            let result = crate::compiler::runtime::Runtime::default()
                .resolve(
                    &mut target,
                    &compilation.program,
                    &crate::compiler::TimeZone::default(),
                )
                .unwrap_or_else(|error| panic!("failed to run `{source}`: {error}"));

            assert_eq!(result, Value::Integer(0), "source: `{source}`");
        }
    }

    #[test]
    fn dynamic_float_arithmetic_preserves_existing_infallible_typing() {
        let sources = [
            (
                "lhs = parse_float!(.lhs); rhs = parse_float!(.rhs); lhs + rhs",
                float(3.0),
            ),
            (
                "lhs = parse_float!(.lhs); rhs = parse_float!(.rhs); lhs - rhs",
                float(-1.0),
            ),
            (
                "lhs = parse_float!(.lhs); rhs = parse_float!(.rhs); lhs * rhs",
                float(2.0),
            ),
            ("lhs = parse_float!(.lhs); lhs / 2", float(0.5)),
            ("lhs = parse_float!(.lhs); mod(lhs, 2.0)", float(1.0)),
        ];
        let target = ObjectMap::from([
            ("lhs".into(), Value::from("1")),
            ("rhs".into(), Value::from("2")),
        ]);

        for (source, expected) in sources {
            let compilation = crate::compiler::compile(source, &crate::stdlib::all())
                .unwrap_or_else(|error| panic!("failed to compile `{source}`: {error:?}"));
            let mut target = Value::Object(target.clone());
            let result = crate::compiler::runtime::Runtime::default()
                .resolve(
                    &mut target,
                    &compilation.program,
                    &crate::compiler::TimeZone::default(),
                )
                .unwrap_or_else(|error| panic!("failed to run `{source}`: {error}"));

            assert_eq!(result, expected, "source: `{source}`");
        }
    }

    #[test]
    fn dynamic_nan_arithmetic_returns_a_runtime_error() {
        let source = "lhs = parse_float!(.lhs); rhs = parse_float!(.rhs); lhs + rhs";
        let compilation = crate::compiler::compile(source, &crate::stdlib::all())
            .unwrap_or_else(|error| panic!("failed to compile `{source}`: {error:?}"));
        let mut target = Value::Object(ObjectMap::from([
            ("lhs".into(), Value::from("inf")),
            ("rhs".into(), Value::from("-inf")),
        ]));
        let error = crate::compiler::runtime::Runtime::default()
            .resolve(
                &mut target,
                &compilation.program,
                &crate::compiler::TimeZone::default(),
            )
            .expect_err("NaN-producing arithmetic must fail");

        assert!(error.to_string().contains("operation would produce NaN"));
    }
}
