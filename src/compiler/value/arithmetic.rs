#![deny(clippy::arithmetic_side_effects)]
#![allow(clippy::cast_precision_loss, clippy::module_name_repetitions)]

use std::ops::{Add, Mul, Rem};

use crate::compiler::{
    ExpressionError,
    value::{Kind, VrlValueConvert},
};
use crate::value::{ObjectMap, Value};
use bytes::{BufMut, Bytes, BytesMut};
use rust_decimal::Decimal;

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

fn safe_sub(lhv: f64, rhv: f64) -> Option<Value> {
    let result = lhv - rhv;
    if result.is_nan() {
        None
    } else {
        Some(Value::from_f64_or_zero(result))
    }
}

impl VrlValueArithmetic for Value {
    /// Similar to [`std::ops::Mul`], but fallible (e.g. `TryMul`).
    fn try_mul(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Mul(self.kind(), rhs.kind());

        // When multiplying a string by an integer, if the number is negative we set it to zero to
        // return an empty string.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let as_usize = |num| if num < 0 { 0 } else { num as usize };

        let value = match self {
            Value::Integer(lhv) if rhs.is_bytes() => {
                Bytes::from(rhs.try_bytes()?.repeat(as_usize(lhv))).into()
            }
            Value::Integer(lhv) if rhs.is_float() => {
                Value::from_f64_or_zero(lhv as f64 * rhs.try_float()?)
            }
            Value::Integer(lhv) if rhs.is_decimal() => {
                let rhv = rhs.try_decimal()?;
                Decimal::from(lhv)
                    .checked_mul(rhv)
                    .map(Value::from)
                    .ok_or(ValueError::Mul(Kind::integer(), Kind::decimal()))?
            }
            Value::Integer(lhv) => {
                let rhv_i64 = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_mul(lhv, rhv_i64).into()
            }
            Value::Float(lhv) => {
                let rhs = rhs.try_into_f64().map_err(|_| err())?;
                lhv.mul(rhs).into()
            }
            Value::Decimal(lhv) => {
                let rhv = rhs
                    .try_into_decimal()
                    .map_err(|_| ValueError::Mul(Kind::decimal(), rhs.kind()))?;
                lhv.checked_mul(rhv)
                    .map(Value::from)
                    .ok_or(ValueError::Mul(Kind::decimal(), Kind::decimal()))?
            }
            Value::Bytes(lhv) if rhs.is_integer() => {
                Bytes::from(lhv.repeat(as_usize(rhs.try_integer()?))).into()
            }
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Div`], but fallible (e.g. `TryDiv`).
    fn try_div(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Div(self.kind(), rhs.kind());

        // Handle Decimal division separately for precision
        if let Value::Decimal(lhv) = &self {
            let rhv = rhs.try_into_decimal().map_err(|_| err())?;
            if rhv.is_zero() {
                return Err(ValueError::DivideByZero);
            }
            return lhv.checked_div(rhv).map(Value::from).ok_or_else(err);
        }

        // Handle Integer / Decimal -> Decimal
        if let (Value::Integer(lhv), Value::Decimal(rhv)) = (&self, &rhs) {
            if rhv.is_zero() {
                return Err(ValueError::DivideByZero);
            }
            return Decimal::from(*lhv)
                .checked_div(*rhv)
                .map(Value::from)
                .ok_or(ValueError::Div(Kind::integer(), Kind::decimal()));
        }

        let rhv_f64 = rhs.try_into_f64().map_err(|_| err())?;

        if rhv_f64 == 0.0 {
            return Err(ValueError::DivideByZero);
        }

        let value = match self {
            Value::Integer(lhv) => Value::from_f64_or_zero(lhv as f64 / rhv_f64),
            Value::Float(lhv) => Value::from_f64_or_zero(lhv.into_inner() / rhv_f64),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Add`], but fallible (e.g. `TryAdd`).
    fn try_add(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Float(rhs)) => Value::from_f64_or_zero(lhs as f64 + *rhs),
            (Value::Integer(lhs), Value::Decimal(rhs)) => Decimal::from(lhs)
                .checked_add(rhs)
                .map(Value::from)
                .ok_or_else(|| ValueError::Add(Kind::integer(), Kind::decimal()))?,
            (Value::Integer(lhs), rhs) => {
                let rhv_i64 = rhs
                    .try_into_i64()
                    .map_err(|_| ValueError::Add(Kind::integer(), rhs.kind()))?;
                i64::wrapping_add(lhs, rhv_i64).into()
            }
            (Value::Float(lhs), rhs) => {
                let rhs = rhs
                    .try_into_f64()
                    .map_err(|_| ValueError::Add(Kind::float(), rhs.kind()))?;
                lhs.add(rhs).into()
            }
            (Value::Decimal(lhs), rhs) => {
                let rhv = rhs
                    .try_into_decimal()
                    .map_err(|_| ValueError::Add(Kind::decimal(), rhs.kind()))?;
                lhs.checked_add(rhv)
                    .map(Value::from)
                    .ok_or_else(|| ValueError::Add(Kind::decimal(), Kind::decimal()))?
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
        let err = || ValueError::Sub(self.kind(), rhs.kind());

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => {
                Value::from_f64_or_zero(lhv as f64 - rhs.try_float()?)
            }
            Value::Integer(lhv) if rhs.is_decimal() => {
                let rhv = rhs.try_decimal()?;
                Decimal::from(lhv)
                    .checked_sub(rhv)
                    .map(Value::from)
                    .ok_or(ValueError::Sub(Kind::integer(), Kind::decimal()))?
            }
            Value::Integer(lhv) => {
                let rhv_i64 = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_sub(lhv, rhv_i64).into()
            }
            Value::Float(lhv) => {
                let rhv = rhs.try_into_f64().map_err(|_| err())?;
                safe_sub(*lhv, rhv).ok_or_else(err)?
            }
            Value::Decimal(lhv) => {
                let rhv = rhs.try_into_decimal().map_err(|_| err())?;
                lhv.checked_sub(rhv)
                    .map(Value::from)
                    .ok_or(ValueError::Sub(Kind::decimal(), Kind::decimal()))?
            }
            _ => return Err(err()),
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
        let err = || ValueError::And(self.kind(), rhs.kind());

        let value = match self {
            Value::Null => false.into(),
            Value::Boolean(left) => match rhs {
                Value::Null => false.into(),
                Value::Boolean(right) => (left && right).into(),
                _ => return Err(err()),
            },
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Rem`], but fallible (e.g. `TryRem`).
    fn try_rem(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Rem(self.kind(), rhs.kind());

        // Handle Decimal separately since try_into_f64 doesn't support Decimal
        if let Value::Decimal(lhv) = &self {
            let right = rhs.try_into_decimal().map_err(|_| err())?;
            if right.is_zero() {
                return Err(ValueError::DivideByZero);
            }
            return lhv
                .checked_rem(right)
                .map(Value::from)
                .ok_or(ValueError::Rem(Kind::decimal(), Kind::decimal()));
        }

        // Handle Integer % Decimal -> Decimal
        if let (Value::Integer(lhv), Value::Decimal(rhv)) = (&self, &rhs) {
            if rhv.is_zero() {
                return Err(ValueError::DivideByZero);
            }
            return Decimal::from(*lhv)
                .checked_rem(*rhv)
                .map(Value::from)
                .ok_or(ValueError::Rem(Kind::integer(), Kind::decimal()));
        }

        let rhv_f64 = rhs.try_into_f64().map_err(|_| err())?;

        if rhv_f64 == 0.0 {
            return Err(ValueError::DivideByZero);
        }

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => {
                Value::from_f64_or_zero(lhv as f64 % rhs.try_float()?)
            }
            Value::Integer(left) => {
                let right = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_rem(left, right).into()
            }
            Value::Float(left) => {
                let right = rhs.try_into_f64().map_err(|_| err())?;
                left.rem(right).into()
            }
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_gt(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Rem(self.kind(), rhs.kind());

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => (lhv as f64 > rhs.try_float()?).into(),
            Value::Integer(lhv) => (lhv > rhs.try_into_i64().map_err(|_| err())?).into(),
            Value::Float(lhv) => (lhv.into_inner() > rhs.try_into_f64().map_err(|_| err())?).into(),
            Value::Decimal(lhv) => (lhv > rhs.try_into_decimal().map_err(|_| err())?).into(),
            Value::Bytes(lhv) => (lhv > rhs.try_bytes()?).into(),
            Value::Timestamp(lhv) => (lhv > rhs.try_timestamp()?).into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_ge(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Ge(self.kind(), rhs.kind());

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => (lhv as f64 >= rhs.try_float()?).into(),
            Value::Integer(lhv) => (lhv >= rhs.try_into_i64().map_err(|_| err())?).into(),
            Value::Float(lhv) => {
                (lhv.into_inner() >= rhs.try_into_f64().map_err(|_| err())?).into()
            }
            Value::Decimal(lhv) => (lhv >= rhs.try_into_decimal().map_err(|_| err())?).into(),
            Value::Bytes(lhv) => (lhv >= rhs.try_bytes()?).into(),
            Value::Timestamp(lhv) => (lhv >= rhs.try_timestamp()?).into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_lt(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Ge(self.kind(), rhs.kind());

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => ((lhv as f64) < rhs.try_float()?).into(),
            Value::Integer(lhv) => (lhv < rhs.try_into_i64().map_err(|_| err())?).into(),
            Value::Float(lhv) => (lhv.into_inner() < rhs.try_into_f64().map_err(|_| err())?).into(),
            Value::Decimal(lhv) => (lhv < rhs.try_into_decimal().map_err(|_| err())?).into(),
            Value::Bytes(lhv) => (lhv < rhs.try_bytes()?).into(),
            Value::Timestamp(lhv) => (lhv < rhs.try_timestamp()?).into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Ord`], but fallible (e.g. `TryOrd`).
    fn try_le(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Ge(self.kind(), rhs.kind());

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => (lhv as f64 <= rhs.try_float()?).into(),
            Value::Integer(lhv) => (lhv <= rhs.try_into_i64().map_err(|_| err())?).into(),
            Value::Float(lhv) => {
                (lhv.into_inner() <= rhs.try_into_f64().map_err(|_| err())?).into()
            }
            Value::Decimal(lhv) => (lhv <= rhs.try_into_decimal().map_err(|_| err())?).into(),
            Value::Bytes(lhv) => (lhv <= rhs.try_bytes()?).into(),
            Value::Timestamp(lhv) => (lhv <= rhs.try_timestamp()?).into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    fn try_merge(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Merge(self.kind(), rhs.kind());

        let value = match (&self, &rhs) {
            (Value::Object(lhv), Value::Object(right)) => lhv
                .iter()
                .chain(right.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<ObjectMap>()
                .into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Eq`], but does a lossless comparison for integers
    /// and floats.
    fn eq_lossy(&self, rhs: &Self) -> bool {
        use Value::{Decimal, Float, Integer};

        match (self, rhs) {
            // Decimal comparisons: convert other to Decimal
            (Decimal(lhv), Decimal(rhv)) => lhv == rhv,
            (Decimal(lhv), Integer(rhv)) => *lhv == rust_decimal::Decimal::from(*rhv),
            (Integer(lhv), Decimal(rhv)) => rust_decimal::Decimal::from(*lhv) == *rhv,
            (Decimal(lhv), Float(rhv)) => rhv
                .into_inner()
                .to_string()
                .parse::<rust_decimal::Decimal>()
                .map(|rhv_d| *lhv == rhv_d)
                .unwrap_or(false),
            (Float(lhv), Decimal(rhv)) => lhv
                .into_inner()
                .to_string()
                .parse::<rust_decimal::Decimal>()
                .map(|lhv_d| lhv_d == *rhv)
                .unwrap_or(false),

            // Float comparisons: convert other to f64
            (Integer(lhv), Float(rhv)) => (*lhv as f64) == rhv.into_inner(),
            (Float(lhv), Integer(rhv)) => lhv.into_inner() == (*rhv as f64),
            (Float(lhv), Float(rhv)) => lhv == rhv,

            // Integer comparisons
            (Integer(lhv), Integer(rhv)) => lhv == rhv,

            // Non-numeric: derived PartialEq (exact type+value match)
            _ => self == rhs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decimal(s: &str) -> Value {
        Value::from(s.parse::<Decimal>().unwrap())
    }

    #[test]
    fn integer_div_decimal_returns_decimal() {
        let result = Value::Integer(10).try_div(decimal("3")).unwrap();
        assert!(result.is_decimal(), "expected Decimal, got {result:?}");
        // 10 / 3 = 3.3333...
        let d = result.as_decimal().unwrap();
        assert!(d > &Decimal::from(3), "10 / 3 should be > 3");
        assert!(d < &Decimal::from(4), "10 / 3 should be < 4");
    }

    #[test]
    fn integer_div_decimal_zero_returns_error() {
        let result = Value::Integer(10).try_div(decimal("0"));
        assert!(result.is_err());
    }

    #[test]
    fn integer_rem_decimal_returns_decimal() {
        // 10 % 3.5 = 3.0
        let result = Value::Integer(10).try_rem(decimal("3.5")).unwrap();
        assert!(result.is_decimal(), "expected Decimal, got {result:?}");
        assert_eq!(result.as_decimal().unwrap(), &Decimal::from(3));
    }

    #[test]
    fn integer_rem_decimal_zero_returns_error() {
        let result = Value::Integer(10).try_rem(decimal("0"));
        assert!(result.is_err());
    }

    #[test]
    fn decimal_add_overflow_returns_error() {
        let max = Value::Decimal(Decimal::MAX);
        let result = max.try_add(decimal("1"));
        assert!(result.is_err());
    }

    #[test]
    fn decimal_sub_overflow_returns_error() {
        let min = Value::Decimal(Decimal::MIN);
        let result = min.try_sub(decimal("1"));
        assert!(result.is_err());
    }

    #[test]
    fn decimal_mul_overflow_returns_error() {
        let max = Value::Decimal(Decimal::MAX);
        let result = max.try_mul(decimal("2"));
        assert!(result.is_err());
    }

    #[test]
    fn decimal_div_zero_returns_error() {
        let result = decimal("1").try_div(decimal("0"));
        assert!(result.is_err());
    }

    #[test]
    fn decimal_rem_zero_returns_error() {
        let result = decimal("1").try_rem(decimal("0"));
        assert!(result.is_err());
    }

    #[test]
    fn float_add_decimal_returns_error() {
        let result = Value::from_f64_or_zero(1.5).try_add(decimal("2.5"));
        assert!(result.is_err(), "Float + Decimal should fail: {result:?}");
    }

    #[test]
    fn float_mul_decimal_returns_error() {
        let result = Value::from_f64_or_zero(2.0).try_mul(decimal("3.0"));
        assert!(result.is_err(), "Float * Decimal should fail: {result:?}");
    }

    #[test]
    fn float_div_decimal_returns_error() {
        let result = Value::from_f64_or_zero(6.0).try_div(decimal("2.0"));
        assert!(result.is_err(), "Float / Decimal should fail: {result:?}");
    }

    #[test]
    fn float_sub_decimal_returns_error() {
        let result = Value::from_f64_or_zero(5.0).try_sub(decimal("1.0"));
        assert!(result.is_err(), "Float - Decimal should fail: {result:?}");
    }

    #[test]
    fn decimal_add_integer_returns_decimal() {
        let result = decimal("1.5").try_add(Value::Integer(2)).unwrap();
        assert!(
            result.is_decimal(),
            "Decimal + Integer should be Decimal: {result:?}"
        );
        assert_eq!(result.as_decimal().unwrap(), &rust_decimal::dec!(3.5));
    }

    #[test]
    fn decimal_sub_integer_returns_decimal() {
        let result = decimal("5.5").try_sub(Value::Integer(2)).unwrap();
        assert!(
            result.is_decimal(),
            "Decimal - Integer should be Decimal: {result:?}"
        );
        assert_eq!(result.as_decimal().unwrap(), &rust_decimal::dec!(3.5));
    }

    #[test]
    fn decimal_mul_integer_returns_decimal() {
        let result = decimal("2.5").try_mul(Value::Integer(4)).unwrap();
        assert!(
            result.is_decimal(),
            "Decimal * Integer should be Decimal: {result:?}"
        );
        assert_eq!(result.as_decimal().unwrap(), &rust_decimal::dec!(10.0));
    }

    #[test]
    fn decimal_div_integer_returns_decimal() {
        let result = decimal("10.0").try_div(Value::Integer(4)).unwrap();
        assert!(
            result.is_decimal(),
            "Decimal / Integer should be Decimal: {result:?}"
        );
        assert_eq!(result.as_decimal().unwrap(), &rust_decimal::dec!(2.5));
    }
}
