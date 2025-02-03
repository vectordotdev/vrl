use bytes::Bytes;
use chrono::{DateTime, Utc};
use ordered_float::NotNan;

use crate::compiler::conversion::{parse_bool, Conversion, Error};
use crate::compiler::TimeZone;

#[cfg(unix)] // see https://github.com/vectordotdev/vector/issues/1201
mod unix;

#[derive(PartialEq, Debug, Clone)]
enum StubValue {
    Bytes(Bytes),
    Timestamp(DateTime<Utc>),
    Float(f64),
    Integer(i64),
    Boolean(bool),
}

impl From<Bytes> for StubValue {
    fn from(v: Bytes) -> Self {
        StubValue::Bytes(v)
    }
}

impl From<DateTime<Utc>> for StubValue {
    fn from(v: DateTime<Utc>) -> Self {
        StubValue::Timestamp(v)
    }
}

impl From<f64> for StubValue {
    fn from(v: f64) -> Self {
        StubValue::Float(v)
    }
}

impl From<NotNan<f64>> for StubValue {
    fn from(v: NotNan<f64>) -> Self {
        StubValue::Float(v.into_inner())
    }
}

impl From<i64> for StubValue {
    fn from(v: i64) -> Self {
        StubValue::Integer(v)
    }
}

impl From<bool> for StubValue {
    fn from(v: bool) -> Self {
        StubValue::Boolean(v)
    }
}

// These should perhaps each go into an individual test function to be
// able to determine what part failed, but that would end up really
// spamming the test logs.

#[test]
fn parse_bool_true() {
    assert_eq!(parse_bool("true"), Ok(true));
    assert_eq!(parse_bool("True"), Ok(true));
    assert_eq!(parse_bool("t"), Ok(true));
    assert_eq!(parse_bool("T"), Ok(true));
    assert_eq!(parse_bool("yes"), Ok(true));
    assert_eq!(parse_bool("YES"), Ok(true));
    assert_eq!(parse_bool("y"), Ok(true));
    assert_eq!(parse_bool("Y"), Ok(true));
    assert_eq!(parse_bool("1"), Ok(true));
    assert_eq!(parse_bool("23456"), Ok(true));
    assert_eq!(parse_bool("-8"), Ok(true));
}

#[test]
fn parse_bool_false() {
    assert_eq!(parse_bool("false"), Ok(false));
    assert_eq!(parse_bool("fAlSE"), Ok(false));
    assert_eq!(parse_bool("f"), Ok(false));
    assert_eq!(parse_bool("F"), Ok(false));
    assert_eq!(parse_bool("no"), Ok(false));
    assert_eq!(parse_bool("NO"), Ok(false));
    assert_eq!(parse_bool("n"), Ok(false));
    assert_eq!(parse_bool("N"), Ok(false));
    assert_eq!(parse_bool("0"), Ok(false));
    assert_eq!(parse_bool("000"), Ok(false));
}

#[test]
fn parse_bool_errors() {
    assert!(parse_bool("X").is_err());
    assert!(parse_bool("yes or no").is_err());
    assert!(parse_bool("123.4").is_err());
}

fn convert_float(input: &str) -> Result<StubValue, Error> {
    let input = input.to_string();
    let converter = Conversion::parse("float", TimeZone::Local).expect("float conversion");
    converter.convert::<StubValue>(input.into())
}

#[test]
fn convert_float_ok() {
    let max_float = format!("17976931348623157{}", "0".repeat(292));
    let min_float = format!("-{max_float}");
    assert_eq!(convert_float(&max_float), Ok(StubValue::Float(f64::MAX)));
    assert_eq!(convert_float("1"), Ok(StubValue::Float(1.0)));
    assert_eq!(convert_float("1.23"), Ok(StubValue::Float(1.23)));
    assert_eq!(convert_float("-1"), Ok(StubValue::Float(-1.0)));
    assert_eq!(convert_float("-1.23"), Ok(StubValue::Float(-1.23)));
    assert_eq!(convert_float(&min_float), Ok(StubValue::Float(f64::MIN)));

    assert_eq!(convert_float("0"), Ok(StubValue::Float(0.0)));
    assert_eq!(convert_float("+0"), Ok(StubValue::Float(0.0)));
    assert_eq!(convert_float("-0"), Ok(StubValue::Float(0.0)));
    assert_eq!(convert_float("0.0"), Ok(StubValue::Float(0.0)));

    let exceeds_max_float = format!("17976931348623159{}", "0".repeat(292));
    let exceeds_min_float = format!("-{exceeds_max_float}");
    assert_eq!(
        convert_float(&exceeds_max_float),
        Ok(StubValue::Float(f64::INFINITY))
    );
    assert_eq!(
        convert_float(&exceeds_min_float),
        Ok(StubValue::Float(f64::NEG_INFINITY))
    );

    let subnormal_lower_than_min = 1.0e-308_f64;
    assert_eq!(
        convert_float(&subnormal_lower_than_min.to_string()),
        Ok(StubValue::Float(1.0e-308_f64))
    );
}
#[test]
fn convert_float_errors() {
    assert!(convert_float("abc").is_err());
    assert!(convert_float("1.23.4").is_err());
}
