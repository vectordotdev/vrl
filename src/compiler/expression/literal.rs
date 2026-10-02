use std::{borrow::Cow, convert::TryFrom, fmt, sync::Arc};

use crate::compiler::codes;
use crate::diagnostic::{DiagnosticMessage, Label, Note, Urls};
use crate::value::{Value, ValueRegex};
use bytes::Bytes;
use bytestring::ByteString;
use chrono::{DateTime, SecondsFormat, Utc};
use ordered_float::NotNan;
use regex::Regex;

use crate::compiler::{
    Context, Expression, Span, TypeDef,
    expression::Resolved,
    state::{TypeInfo, TypeState},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(ByteString),
    Bytes(Bytes),
    Integer(i64),
    Float(NotNan<f64>),
    Boolean(bool),
    Regex(ValueRegex),
    Timestamp(DateTime<Utc>),
    Null,
}

impl Literal {
    /// Get a `Value` type stored in the literal.
    ///
    /// This differs from `Expression::as_value` insofar as this *always*
    /// returns a `Value`, whereas `as_value` returns `Option<Value>` which, in
    /// the case of `Literal` means it always returns `Some(Value)`, requiring
    /// an extra `unwrap()`.
    pub fn to_value(&self) -> Value {
        use Literal::{Boolean, Bytes, Float, Integer, Null, Regex, String, Timestamp};

        match self {
            String(v) => Value::String(v.clone()),
            Bytes(v) => Value::Bytes(v.clone()),
            Integer(v) => Value::Integer(*v),
            Float(v) => Value::Float(*v),
            Boolean(v) => Value::Boolean(*v),
            Regex(v) => Value::Regex(v.clone()),
            Timestamp(v) => Value::Timestamp(*v),
            Null => Value::Null,
        }
    }

    /// Build a UTF-8 string literal from bytes that are UTF-8 by construction.
    ///
    /// # Safety
    ///
    /// `bytes` must be valid UTF-8. See [`Value::from_utf8_unchecked`].
    pub unsafe fn from_utf8_unchecked(bytes: Bytes) -> Self {
        // SAFETY: caller must uphold this function's safety contract.
        let Value::String(v) = (unsafe { Value::from_utf8_unchecked(bytes) }) else {
            unreachable!("from_utf8_unchecked always returns Value::String");
        };
        Self::String(v)
    }
}

impl Expression for Literal {
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(self.to_value())
    }

    fn resolve_constant(&self, _state: &TypeState) -> Option<Value> {
        Some(self.to_value())
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        use Literal::{Boolean, Bytes, Float, Integer, Null, Regex, String, Timestamp};

        let type_def = match self {
            String(_) | Bytes(_) => TypeDef::bytes(),
            Integer(_) => TypeDef::integer(),
            Float(_) => TypeDef::float(),
            Boolean(_) => TypeDef::boolean(),
            Regex(_) => TypeDef::regex(),
            Timestamp(_) => TypeDef::timestamp(),
            Null => TypeDef::null(),
        };

        TypeInfo::new(state, type_def.infallible())
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Literal::{Boolean, Bytes, Float, Integer, Null, Regex, String, Timestamp};

        match self {
            String(v) => write!(f, r#""{}""#, &**v),
            Bytes(v) => write!(f, r#""{}""#, std::string::String::from_utf8_lossy(v)),
            Integer(v) => v.fmt(f),
            Float(v) => v.fmt(f),
            Boolean(v) => v.fmt(f),
            Regex(v) => v.fmt(f),
            Timestamp(v) => write!(f, "t'{}'", v.to_rfc3339_opts(SecondsFormat::AutoSi, true)),
            Null => f.write_str("null"),
        }
    }
}

// Literal::String / Literal::Bytes --------------------------------------------

impl From<Bytes> for Literal {
    fn from(v: Bytes) -> Self {
        Literal::Bytes(v)
    }
}

impl From<ByteString> for Literal {
    fn from(v: ByteString) -> Self {
        Literal::String(v)
    }
}

impl From<Cow<'_, str>> for Literal {
    fn from(v: Cow<'_, str>) -> Self {
        v.as_ref().into()
    }
}

impl From<Vec<u8>> for Literal {
    fn from(v: Vec<u8>) -> Self {
        Literal::Bytes(Bytes::from(v))
    }
}

impl From<&[u8]> for Literal {
    fn from(v: &[u8]) -> Self {
        Literal::Bytes(Bytes::copy_from_slice(v))
    }
}

impl From<String> for Literal {
    fn from(v: String) -> Self {
        Literal::String(v.into())
    }
}

impl From<&str> for Literal {
    fn from(v: &str) -> Self {
        Literal::String(v.into())
    }
}

// Literal::Integer ------------------------------------------------------------

impl From<i8> for Literal {
    fn from(v: i8) -> Self {
        Literal::Integer(i64::from(v))
    }
}

impl From<i16> for Literal {
    fn from(v: i16) -> Self {
        Literal::Integer(i64::from(v))
    }
}

impl From<i32> for Literal {
    fn from(v: i32) -> Self {
        Literal::Integer(i64::from(v))
    }
}

impl From<i64> for Literal {
    fn from(v: i64) -> Self {
        Literal::Integer(v)
    }
}

impl From<u16> for Literal {
    fn from(v: u16) -> Self {
        Literal::Integer(i64::from(v))
    }
}

impl From<u32> for Literal {
    fn from(v: u32) -> Self {
        Literal::Integer(i64::from(v))
    }
}

impl From<u64> for Literal {
    fn from(v: u64) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Literal::Integer(v as i64)
    }
}

impl From<usize> for Literal {
    fn from(v: usize) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Literal::Integer(v as i64)
    }
}

// Literal::Float --------------------------------------------------------------

impl From<NotNan<f64>> for Literal {
    fn from(v: NotNan<f64>) -> Self {
        Literal::Float(v)
    }
}

impl TryFrom<f64> for Literal {
    type Error = Error;

    fn try_from(v: f64) -> Result<Self, Self::Error> {
        Ok(Literal::Float(NotNan::new(v).map_err(|_| Error {
            span: Span::default(),
            variant: ErrorVariant::NanFloat,
        })?))
    }
}

// Literal::Boolean ------------------------------------------------------------

impl From<bool> for Literal {
    fn from(v: bool) -> Self {
        Literal::Boolean(v)
    }
}

// Literal::Regex --------------------------------------------------------------

impl From<Arc<Regex>> for Literal {
    fn from(regex: Arc<Regex>) -> Self {
        Literal::Regex(ValueRegex::new(regex))
    }
}

impl From<ValueRegex> for Literal {
    fn from(regex: ValueRegex) -> Self {
        Literal::Regex(regex)
    }
}

// Literal::Null ---------------------------------------------------------------

impl From<()> for Literal {
    fn from((): ()) -> Self {
        Literal::Null
    }
}

impl<T: Into<Literal>> From<Option<T>> for Literal {
    fn from(literal: Option<T>) -> Self {
        match literal {
            None => Literal::Null,
            Some(v) => v.into(),
        }
    }
}

// Literal::Regex --------------------------------------------------------------

impl From<DateTime<Utc>> for Literal {
    fn from(dt: DateTime<Utc>) -> Self {
        Literal::Timestamp(dt)
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct Error {
    pub(crate) variant: ErrorVariant,
    span: Span,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ErrorVariant {
    #[error("invalid regular expression")]
    InvalidRegex(#[from] regex::Error),

    #[error("invalid timestamp")]
    InvalidTimestamp(#[from] chrono::ParseError),

    #[error("float literal can't be NaN")]
    NanFloat,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.variant)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.variant)
    }
}

impl DiagnosticMessage for Error {
    fn code(&self) -> usize {
        use ErrorVariant::{InvalidRegex, InvalidTimestamp, NanFloat};

        match &self.variant {
            InvalidRegex(..) => codes::ExprCode::InvalidRegex as usize,
            InvalidTimestamp(..) => codes::CompilerCode::InvalidTimestamp as usize,
            NanFloat => codes::CompilerCode::NanFloatLiteral as usize,
        }
    }

    fn labels(&self) -> Vec<Label> {
        use ErrorVariant::{InvalidRegex, InvalidTimestamp, NanFloat};

        match &self.variant {
            InvalidRegex(err) => {
                let error = err
                    .to_string()
                    .lines()
                    .filter_map(|line| {
                        if line.trim() == "^" || line == "regex parse error:" {
                            return None;
                        }

                        Some(line.trim_start_matches("error: ").trim())
                    })
                    .rev()
                    .collect::<Vec<_>>()
                    .join(": ");

                vec![Label::primary(
                    format!("regex parse error: {error}"),
                    self.span,
                )]
            }
            InvalidTimestamp(err) => vec![Label::primary(
                format!("invalid timestamp format: {err}"),
                self.span,
            )],

            NanFloat => vec![],
        }
    }

    fn notes(&self) -> Vec<Note> {
        use ErrorVariant::{InvalidRegex, InvalidTimestamp, NanFloat};

        match &self.variant {
            InvalidRegex(_) => vec![Note::SeeDocs(
                "regular expressions".to_owned(),
                Urls::expression_docs_url("#regular-expression"),
            )],
            InvalidTimestamp(_) => vec![Note::SeeDocs(
                "timestamps".to_owned(),
                Urls::expression_docs_url("#timestamp"),
            )],
            NanFloat => vec![Note::SeeDocs(
                "floats".to_owned(),
                Urls::expression_docs_url("#float"),
            )],
        }
    }
}

impl From<(Span, regex::Error)> for Error {
    fn from((span, err): (Span, regex::Error)) -> Self {
        Self {
            variant: err.into(),
            span,
        }
    }
}

impl From<(Span, chrono::ParseError)> for Error {
    fn from((span, err): (Span, chrono::ParseError)) -> Self {
        Self {
            variant: err.into(),
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::TypeDef;
    use crate::compiler::expression::Expr;
    use crate::value::Value;
    use crate::{expr, test_type_def};
    use bytes::Bytes;

    test_type_def![
        bytes {
            expr: |_| expr!("foo"),
            want: TypeDef::bytes(),
        }

        integer {
            expr: |_| expr!(12),
            want: TypeDef::integer(),
        }
    ];

    #[test]
    fn value_round_trip_does_not_promote_bytes() {
        let utf8 = Value::from_static_bytes("foo");
        let Expr::Literal(literal) = Expr::from(utf8.clone()) else {
            panic!("expected literal");
        };
        assert!(matches!(literal.to_value(), Value::Bytes(_)));
        assert_eq!(literal.to_value(), utf8);

        let raw = Value::Bytes(Bytes::from_static(b"foo\xff"));
        let Expr::Literal(literal) = Expr::from(raw.clone()) else {
            panic!("expected literal");
        };
        assert!(matches!(literal.to_value(), Value::Bytes(_)));
        assert_eq!(literal.to_value(), raw);
    }

    #[test]
    fn value_round_trip_preserves_string() {
        let string = Value::from("foo");
        assert!(matches!(string, Value::String(_)));
        let Expr::Literal(literal) = Expr::from(string.clone()) else {
            panic!("expected literal");
        };
        assert!(matches!(literal.to_value(), Value::String(_)));
        assert_eq!(literal.to_value(), string);
    }
}
