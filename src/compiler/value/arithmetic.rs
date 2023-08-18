#![deny(clippy::arithmetic_side_effects)]

use std::collections::BTreeMap;
use std::ops::{Add, Mul, Rem, Sub};

use crate::value::Value;
use bytes::{BufMut, Bytes, BytesMut};

use super::ValueError;
use crate::compiler::{
    value::{Kind, VrlValueConvert},
    ExpressionError,
};

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

impl VrlValueArithmetic for Value {
    /// Similar to [`std::ops::Mul`], but fallible (e.g. `TryMul`).
    fn try_mul(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Mul(self.kind(), rhs.kind());

        // When multiplying a string by an integer, if the number is negative we set it to zero to
        // return an empty string.
        let as_usize = |num| if num < 0 { 0 } else { num as usize };

        let value = match self {
            Value::Integer(lhv) if rhs.is_bytes() => {
                Bytes::from(rhs.try_bytes()?.repeat(as_usize(lhv))).into()
            }
            Value::Integer(lhv) if rhs.is_float() => {
                Value::from_f64_or_zero(lhv as f64 * rhs.try_float()?)
            }
            Value::Integer(lhv) => {
                let rhv = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_mul(lhv, rhv).into()
            }
            Value::Float(lhv) => {
                let rhs = rhs.try_into_f64().map_err(|_| err())?;
                lhv.mul(rhs).into()
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

        let rhv = rhs.try_into_f64().map_err(|_| err())?;

        if rhv == 0.0 {
            return Err(ValueError::DivideByZero);
        }

        let value = match self {
            Value::Integer(lhv) => Value::from_f64_or_zero(lhv as f64 / rhv),
            Value::Float(lhv) => Value::from_f64_or_zero(lhv.into_inner() / rhv),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Add`], but fallible (e.g. `TryAdd`).
    fn try_add(self, rhs: Self) -> Result<Self, ValueError> {
        let value = match (self, rhs) {
            (Value::Integer(lhs), Value::Float(rhs)) => Value::from_f64_or_zero(lhs as f64 + *rhs),
            (Value::Integer(lhs), rhs) => {
                let rhv = rhs
                    .try_into_i64()
                    .map_err(|_| ValueError::Add(Kind::integer(), rhs.kind()))?;
                i64::wrapping_add(lhs, rhv).into()
            }
            (Value::Float(lhs), rhs) => {
                let rhs = rhs
                    .try_into_f64()
                    .map_err(|_| ValueError::Add(Kind::float(), rhs.kind()))?;
                lhs.add(rhs).into()
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
            Value::Integer(lhv) => {
                let rhv = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_sub(lhv, rhv).into()
            }
            Value::Float(lhv) => {
                let rhv = rhs.try_into_f64().map_err(|_| err())?;
                lhv.sub(rhv).into()
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
            Value::Null => rhs().map_err(err),
            Value::Boolean(false) => rhs().map_err(err),
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
            Value::Boolean(lhv) => match rhs {
                Value::Null => false.into(),
                Value::Boolean(rhv) => (lhv && rhv).into(),
                _ => return Err(err()),
            },
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::ops::Rem`], but fallible (e.g. `TryRem`).
    fn try_rem(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Rem(self.kind(), rhs.kind());

        let rhv = rhs.try_into_f64().map_err(|_| err())?;

        if rhv == 0.0 {
            return Err(ValueError::DivideByZero);
        }

        let value = match self {
            Value::Integer(lhv) if rhs.is_float() => {
                Value::from_f64_or_zero(lhv as f64 % rhs.try_float()?)
            }
            Value::Integer(lhv) => {
                let rhv = rhs.try_into_i64().map_err(|_| err())?;
                i64::wrapping_rem(lhv, rhv).into()
            }
            Value::Float(lhv) => {
                let rhv = rhs.try_into_f64().map_err(|_| err())?;
                lhv.rem(rhv).into()
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
            Value::Bytes(lhv) => (lhv > rhs.try_bytes()?).into(),
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
            Value::Bytes(lhv) => (lhv >= rhs.try_bytes()?).into(),
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
            Value::Bytes(lhv) => (lhv < rhs.try_bytes()?).into(),
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
            Value::Bytes(lhv) => (lhv <= rhs.try_bytes()?).into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    fn try_merge(self, rhs: Self) -> Result<Self, ValueError> {
        let err = || ValueError::Merge(self.kind(), rhs.kind());

        let value = match (&self, &rhs) {
            (Value::Object(lhv), Value::Object(rhv)) => lhv
                .iter()
                .chain(rhv.iter())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<String, Value>>()
                .into(),
            _ => return Err(err()),
        };

        Ok(value)
    }

    /// Similar to [`std::cmp::Eq`], but does a lossless comparison for integers
    /// and floats.
    fn eq_lossy(&self, rhs: &Self) -> bool {
        use Value::{Float, Integer};

        match self {
            Integer(lhv) => rhs
                .try_into_f64()
                .map(|rhv| *lhv as f64 == rhv)
                .unwrap_or(false),

            Float(lhv) => rhs
                .try_into_f64()
                .map(|rhv| lhv.into_inner() == rhv)
                .unwrap_or(false),

            _ => self == rhs,
        }
    }
}
