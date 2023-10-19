use std::fmt::Debug;

use chrono::format::{parse, Parsed, StrftimeItems};
use chrono::{DateTime, FixedOffset, Local, Offset, ParseError, TimeZone as _, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// Timezone reference.
///
/// This can refer to any valid timezone as defined in the [TZ database][tzdb], or "local" which
/// refers to the system local timezone.
///
/// [tzdb]: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub enum TimeZone {
    /// System local timezone.
    #[default]
    Local,

    /// A named timezone.
    ///
    /// Must be a valid name in the [TZ database][tzdb].
    ///
    /// [tzdb]: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones
    Named(Tz),
}

/// This is a wrapper trait to allow `TimeZone` types to be passed generically.
impl TimeZone {
    /// Parse a date/time string into `DateTime<Utc>`.
    ///
    /// # Errors
    ///
    /// Returns parse errors from the underlying time parsing functions.
    pub fn datetime_from_str(&self, s: &str, format: &str) -> Result<DateTime<Utc>, ParseError> {
        let mut parsed = Parsed::new();
        parse(&mut parsed, s, StrftimeItems::new(format))?;

        match self {
            Self::Local => {
                let local_datetime = parsed.to_datetime_with_timezone(&Local)?;
                Ok(datetime_to_utc(&local_datetime))
            }
            Self::Named(tz) => {
                let tz_datetime = parsed.to_datetime_with_timezone(tz)?;
                Ok(datetime_to_utc(&tz_datetime))
            }
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "" | "local" => Some(Self::Local),
            _ => s.parse::<Tz>().ok().map(Self::Named),
        }
    }
}

/// Convert a timestamp with a non-UTC time zone into UTC
pub(super) fn datetime_to_utc<TZ: chrono::TimeZone>(ts: &DateTime<TZ>) -> DateTime<Utc> {
    Utc.timestamp_opt(ts.timestamp(), ts.timestamp_subsec_nanos())
        .single()
        .expect("invalid timestamp")
}

impl From<TimeZone> for String {
    fn from(tz: TimeZone) -> Self {
        match tz {
            TimeZone::Local => "local".to_string(),
            TimeZone::Named(tz) => tz.name().to_string(),
        }
    }
}

impl TryFrom<String> for TimeZone {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match TimeZone::parse(&value) {
            Some(tz) => Ok(tz),
            None => Err("No such time zone".to_string()),
        }
    }
}

impl From<TimeZone> for FixedOffset {
    fn from(tz: TimeZone) -> Self {
        match tz {
            TimeZone::Local => *Utc::now().with_timezone(&Local).offset(),
            TimeZone::Named(tz) => Utc::now().with_timezone(&tz).offset().fix(),
        }
    }
}
