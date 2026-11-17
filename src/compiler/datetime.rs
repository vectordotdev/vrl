use jiff::{Timestamp, civil, tz};
use serde::{Deserialize, Serialize};

mod strftime;

pub use strftime::{
    FormatError, ParseError, format, parse, to_naive_datetime, to_offset_timestamp, to_timestamp,
};

/// The current civil date and time in UTC.
#[must_use]
pub fn utc_now() -> civil::DateTime {
    tz::Offset::UTC.to_datetime(Timestamp::now())
}

/// Timezone reference.
///
/// This can refer to any valid timezone as defined in the [TZ database][tzdb], or "local" which
/// refers to the system local timezone.
///
/// [tzdb]: https://en.wikipedia.org/wiki/List_of_tz_database_time_zones
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
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
    Named(tz::TimeZone),
}

impl TimeZone {
    /// Returns the `jiff` time zone this reference resolves to.
    ///
    /// `Local` resolves to the system time zone at the time of the call.
    #[must_use]
    pub fn to_jiff(&self) -> tz::TimeZone {
        match self {
            Self::Local => tz::TimeZone::system(),
            Self::Named(tz) => tz.clone(),
        }
    }

    /// Parse a date/time string with a [chrono-compatible format][format] into a `Timestamp`.
    ///
    /// Values without a UTC offset are interpreted in this time zone.
    ///
    /// # Errors
    ///
    /// Returns an error if `s` does not match `format`, if `format` lacks the date or time fields,
    /// or if the local time does not exist (or is ambiguous) in this time zone.
    ///
    /// [format]: https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers
    pub fn datetime_from_str(&self, s: &str, format: &str) -> Result<Timestamp, ParseError> {
        to_timestamp(&parse(s, format)?, &self.to_jiff())
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "" | "local" => Some(Self::Local),
            _ => tz::TimeZone::get(s).ok().map(Self::Named),
        }
    }
}

impl From<TimeZone> for String {
    fn from(tz: TimeZone) -> Self {
        match tz {
            TimeZone::Local => "local".to_string(),
            // Time zones without an IANA name (fixed offsets) cannot be parsed back, so use the
            // name `jiff` gives them; deserializing it fails instead of picking a wrong zone.
            TimeZone::Named(tz) => tz.iana_name().unwrap_or("Etc/Unknown").to_string(),
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
