use bytes::Bytes;
use chrono::{DateTime, Utc};
use chrono_tz::{Australia, Tz};
use ordered_float::NotNan;

use crate::compiler::{
    conversion::{Conversion, Error, parse_timestamp, tests::StubValue},
    datetime::TimeZone,
};

const TIMEZONE_NAME: &str = "Australia/Brisbane";
const TIMEZONE: Tz = Australia::Brisbane;

#[test]
fn parse_timestamp_auto() {
    let good = Ok(dateref());
    let tz = TimeZone::Named(TIMEZONE);
    assert_eq!(parse_timestamp(tz, "2001-02-03 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "02/03/2001:14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-03T14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-03T04:05:06Z"), good);
    assert_eq!(parse_timestamp(tz, "Sat, 3 Feb 2001 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "Sat Feb 3 14:05:06 2001"), good);
    assert_eq!(parse_timestamp(tz, "3-Feb-2001 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-02T22:05:06-06:00"), good);
    assert_eq!(parse_timestamp(tz, "Sat, 03 Feb 2001 07:05:06 +0300"), good);
    assert_eq!(parse_timestamp(tz, "981173106"), good);
}

#[test]
fn parse_timestamp_auto_tz_env() {
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("TZ", TIMEZONE_NAME) };
    let good = Ok(dateref());
    let tz = TimeZone::Local;
    assert_eq!(parse_timestamp(tz, "2001-02-03 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "02/03/2001:14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-03T14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-03T04:05:06Z"), good);
    assert_eq!(parse_timestamp(tz, "Sat, 3 Feb 2001 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "Sat Feb 3 14:05:06 2001"), good);
    assert_eq!(parse_timestamp(tz, "3-Feb-2001 14:05:06"), good);
    assert_eq!(parse_timestamp(tz, "2001-02-02T22:05:06-06:00"), good);
    assert_eq!(parse_timestamp(tz, "Sat, 03 Feb 2001 07:05:06 +0300"), good);
    assert_eq!(parse_timestamp(tz, "03/Feb/2001:02:05:06 -0200"), good);
    assert_eq!(parse_timestamp(tz, "981173106"), good);
}

#[test]
fn timestamp_param_conversion() {
    assert_eq!(
        convert::<StubValue>("timestamp|%Y-%m-%d %H:%M:%S", "2001-02-03 14:05:06"),
        Ok(dateref().into())
    );
}

fn dateref() -> DateTime<Utc> {
    DateTime::from_timestamp(981_173_106, 0).expect("invalid timestamp")
}

#[allow(clippy::trait_duplication_in_bounds)] // appears to be a false positive
fn convert<T>(fmt: &str, value: &'static str) -> Result<T, Error>
where
    T: From<Bytes> + From<i64> + From<NotNan<f64>> + From<bool> + From<DateTime<Utc>>,
{
    // TODO: Audit that the environment access only happens in single-threaded code.
    unsafe { std::env::set_var("TZ", TIMEZONE_NAME) };
    Conversion::parse(fmt, TimeZone::Local)
        .unwrap_or_else(|_| panic!("Invalid conversion {fmt:?}"))
        .convert(value.into())
}

#[test]
fn timestamp_conversion() {
    assert_eq!(
        convert::<StubValue>("timestamp", "02/03/2001:14:05:06"),
        Ok(dateref().into())
    );
}
