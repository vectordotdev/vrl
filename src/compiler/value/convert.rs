use std::borrow::Cow;

use crate::value::{kind::Collection, Value, ValueRegex};
use bytes::Bytes;
use chrono::{DateTime, Utc};

use crate::compiler::{
    expression::{Container, Expr, Variant},
    value::{Kind, ObjectMap, ValueError},
    Expression,
};

pub trait VrlValueConvert: Sized {
    /// Convert a given [`Value`] into a [`Expression`] trait object.
    fn into_expression(self) -> Box<dyn Expression>;

    fn try_integer(self) -> Result<i64, ValueError>;
    fn try_float(self) -> Result<f64, ValueError>;
    fn try_bytes(self) -> Result<Bytes, ValueError>;
    fn try_boolean(self) -> Result<bool, ValueError>;
    fn try_regex(self) -> Result<ValueRegex, ValueError>;
    fn try_null(self) -> Result<(), ValueError>;
    fn try_array(self) -> Result<Vec<Value>, ValueError>;
    fn try_object(self) -> Result<ObjectMap, ValueError>;
    fn try_timestamp(self) -> Result<DateTime<Utc>, ValueError>;

    fn try_into_i64(&self) -> Result<i64, ValueError>;
    fn try_into_f64(&self) -> Result<f64, ValueError>;

    fn try_bytes_utf8_lossy(&self) -> Result<Cow<'_, str>, ValueError>;
}

impl VrlValueConvert for Value {
    /// Convert a given [`Value`] into a [`Expression`] trait object.
    fn into_expression(self) -> Box<dyn Expression> {
        Box::new(Expr::from(self))
    }

    fn try_integer(self) -> Result<i64, ValueError> {
        match self {
            Value::Integer(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::integer(),
            }),
        }
    }

    fn try_into_i64(self: &Value) -> Result<i64, ValueError> {
        match self {
            Value::Integer(v) => Ok(*v),
            Value::Float(v) => Ok(v.into_inner() as i64),
            _ => Err(ValueError::Coerce(self.kind(), Kind::integer())),
        }
    }

    fn try_float(self) -> Result<f64, ValueError> {
        match self {
            Value::Float(v) => Ok(v.into_inner()),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::float(),
            }),
        }
    }

    fn try_into_f64(&self) -> Result<f64, ValueError> {
        match self {
            Value::Integer(v) => Ok(*v as f64),
            Value::Float(v) => Ok(v.into_inner()),
            _ => Err(ValueError::Coerce(self.kind(), Kind::float())),
        }
    }

    fn try_bytes(self) -> Result<Bytes, ValueError> {
        match self {
            Value::Bytes(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::bytes(),
            }),
        }
    }

    fn try_bytes_utf8_lossy(&self) -> Result<Cow<'_, str>, ValueError> {
        match self.as_bytes() {
            Some(bytes) => Ok(String::from_utf8_lossy(bytes)),
            None => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::bytes(),
            }),
        }
    }

    fn try_boolean(self) -> Result<bool, ValueError> {
        match self {
            Value::Boolean(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::boolean(),
            }),
        }
    }

    fn try_regex(self) -> Result<ValueRegex, ValueError> {
        match self {
            Value::Regex(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::regex(),
            }),
        }
    }

    fn try_null(self) -> Result<(), ValueError> {
        match self {
            Value::Null => Ok(()),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::null(),
            }),
        }
    }

    fn try_array(self) -> Result<Vec<Value>, ValueError> {
        match self {
            Value::Array(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::array(Collection::any()),
            }),
        }
    }

    fn try_object(self) -> Result<ObjectMap, ValueError> {
        match self {
            Value::Object(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::object(Collection::any()),
            }),
        }
    }

    fn try_timestamp(self) -> Result<DateTime<Utc>, ValueError> {
        match self {
            Value::Timestamp(v) => Ok(v),
            _ => Err(ValueError::Expected {
                got: self.kind(),
                expected: Kind::timestamp(),
            }),
        }
    }
}

/// Converts from an `Expr` into a `Value`. This is only possible if the expression represents
/// static values - `Literal`s and `Container`s containing `Literal`s.
/// The error returns the expression back so it can be used in the error report.
impl TryFrom<Expr> for Value {
    type Error = Expr;

    fn try_from(expr: Expr) -> Result<Self, Self::Error> {
        match expr {
            Expr::Literal(literal) => Ok(literal.to_value()),
            Expr::Container(Container {
                variant: Variant::Object(object),
            }) => Ok(Value::Object(
                object
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), value.clone().try_into()?)))
                    .collect::<Result<_, Self::Error>>()?,
            )),
            Expr::Container(Container {
                variant: Variant::Array(array),
            }) => Ok(Value::Array(
                array
                    .iter()
                    .map(|value| value.clone().try_into())
                    .collect::<Result<_, _>>()?,
            )),
            expr => Err(expr),
        }
    }
}
