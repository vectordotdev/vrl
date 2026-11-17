//! Formatting and parsing with chrono-compatible `strftime` format strings.
//!
//! VRL format strings follow [chrono's specifiers][chrono]. Most of them have the same meaning in
//! [`jiff::fmt::strtime`], so this module hands those to `jiff` unchanged and implements the
//! differences itself:
//!
//! | Specifier                  | chrono behavior                                                  |
//! | -------------------------- | ---------------------------------------------------------------- |
//! | `%f`                       | Formats 9 digits; parses up to 9 digits as integer nanoseconds.  |
//! | `%.f`                      | Formats 0, 3, 6 or 9 digits, whichever is the shortest exact one. |
//! | `%+`                       | `%Y-%m-%dT%H:%M:%S%.f%:z`; parses relaxed RFC 3339.               |
//! | `%z`, `%:z`, `%::z`, `%:::z` | Parse an offset with an optional colon.                        |
//! | `%#z`                      | Parses an offset with optional colon and minutes, or `Z`.        |
//! | `%Z`                       | Parsing skips the time zone name.                                |
//! | `%v`, `%c`, `%x`, `%X`, `%r` | Fixed POSIX-locale expansions.                                 |
//!
//! [chrono]: https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers

use std::fmt::{self, Write as _};

use jiff::{
    SignedDuration, Timestamp, civil,
    fmt::strtime::BrokenDownTime,
    tz::{self, AmbiguousOffset, Offset},
};

use crate::value::value::timestamp_seconds;

const OUT_OF_RANGE: &str = "input is out of range";
const IMPOSSIBLE: &str = "no possible date and time matching input";
const NOT_ENOUGH: &str = "input is not enough for unique date and time";
const INVALID: &str = "input contains invalid characters";
const TOO_SHORT: &str = "premature end of input";
const TOO_LONG: &str = "trailing input";
const BAD_FORMAT: &str = "bad or unsupported format string";

/// An error from parsing a date/time string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError(String);

impl ParseError {
    fn new(message: &str) -> Self {
        Self(message.to_owned())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl From<jiff::Error> for ParseError {
    fn from(error: jiff::Error) -> Self {
        Self(error.to_string())
    }
}

/// An error from formatting a timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormatError(String);

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormatError {}

impl From<jiff::Error> for FormatError {
    fn from(error: jiff::Error) -> Self {
        Self(error.to_string())
    }
}

/// A conversion specifier: `%[flag][width][colons][.width]conversion`.
struct Spec {
    flag: Option<u8>,
    width: bool,
    colons: usize,
    dot: bool,
    conversion: char,
}

impl Spec {
    /// Parses the specifier starting at the `%` at `start`. Returns it with the end offset.
    fn parse(format: &str, start: usize) -> Option<(Self, usize)> {
        let bytes = format.as_bytes();
        let mut i = start + 1;
        let flag = match bytes.get(i) {
            Some(&flag @ (b'-' | b'_' | b'0' | b'^' | b'#')) => {
                i += 1;
                Some(flag)
            }
            _ => None,
        };
        let digits_start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let mut width = i > digits_start;
        let colons_start = i;
        while bytes.get(i) == Some(&b':') {
            i += 1;
        }
        let colons = i - colons_start;
        let dot = bytes.get(i) == Some(&b'.');
        if dot {
            i += 1;
            let digits_start = i;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            width |= i > digits_start;
        }
        let conversion = format.get(i..)?.chars().next()?;
        let spec = Self {
            flag,
            width,
            colons,
            dot,
            conversion,
        };
        Some((spec, i + conversion.len_utf8()))
    }

    fn is_plain(&self) -> bool {
        self.flag.is_none() && !self.width && self.colons == 0 && !self.dot
    }

    /// The fixed POSIX-locale expansion of this specifier, if it has one.
    ///
    /// When parsing, `%D` is expanded too, so that its `%y` follows chrono's rules.
    fn expansion(&self, parsing: bool) -> Option<&'static str> {
        if !self.is_plain() {
            return None;
        }
        match self.conversion {
            'v' => Some("%e-%b-%Y"),
            'c' => Some("%a %b %e %H:%M:%S %Y"),
            'x' => Some("%m/%d/%y"),
            'X' => Some("%H:%M:%S"),
            'r' => Some("%I:%M:%S %p"),
            'D' if parsing => Some("%m/%d/%y"),
            _ => None,
        }
    }
}

/// A piece of a format string, as produced by [`walk`].
enum Piece<'f, T> {
    /// Format text that `jiff` handles unchanged.
    Jiff(&'f str),
    /// A specifier that `classify` mapped to `T`.
    Special(T),
}

/// Splits `format` into pieces and passes them to `visit` in order.
fn walk<'f, T, E>(
    format: &'f str,
    classify: impl Fn(&Spec) -> Option<T>,
    bad_format: impl Fn() -> E,
    mut visit: impl FnMut(Piece<'f, T>) -> Result<(), E>,
) -> Result<(), E> {
    let bytes = format.as_bytes();
    let mut jiff_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        let (spec, end) = Spec::parse(format, i).ok_or_else(&bad_format)?;
        if let Some(item) = classify(&spec) {
            visit(Piece::Jiff(&format[jiff_start..i]))?;
            visit(Piece::Special(item))?;
            jiff_start = end;
        }
        i = end;
    }
    visit(Piece::Jiff(&format[jiff_start..]))
}

enum FormatItem {
    /// Format text to use instead of the specifier.
    Expand(&'static str),
    /// `%.f`: the shortest of 0, 3, 6 or 9 digits that is exact.
    AutoFraction,
    /// `%s`: seconds rounded towards negative infinity.
    Seconds,
    /// `%+`
    Rfc3339,
}

fn classify_format(spec: &Spec) -> Option<FormatItem> {
    if let Some(expansion) = spec.expansion(false) {
        return Some(FormatItem::Expand(expansion));
    }
    match spec.conversion {
        'f' if spec.is_plain() => Some(FormatItem::Expand("%9f")),
        'f' if spec.dot && !spec.width && spec.flag.is_none() && spec.colons == 0 => {
            Some(FormatItem::AutoFraction)
        }
        's' if spec.is_plain() => Some(FormatItem::Seconds),
        '+' if spec.is_plain() => Some(FormatItem::Rfc3339),
        _ => None,
    }
}

fn auto_fraction(nanosecond: i32) -> &'static str {
    if nanosecond == 0 {
        ""
    } else if nanosecond % 1_000_000 == 0 {
        "%.3f"
    } else if nanosecond % 1_000 == 0 {
        "%.6f"
    } else {
        "%.9f"
    }
}

struct Formatter {
    tm: BrokenDownTime,
    /// The `%.f` format for the formatted value.
    fraction: &'static str,
    seconds: i64,
    out: String,
}

impl Formatter {
    fn write(&mut self, format: &str) -> Result<(), FormatError> {
        walk(
            format,
            classify_format,
            || FormatError(BAD_FORMAT.to_owned()),
            |piece| match piece {
                Piece::Jiff(format) => Ok(self.tm.format(format, &mut self.out)?),
                Piece::Special(FormatItem::Expand(format)) => self.write(format),
                Piece::Special(FormatItem::AutoFraction) => self.write(self.fraction),
                Piece::Special(FormatItem::Seconds) => {
                    write!(self.out, "{}", self.seconds).expect("writing to a String");
                    Ok(())
                }
                Piece::Special(FormatItem::Rfc3339) => {
                    self.write("%Y-%m-%dT%H:%M:%S")?;
                    self.write(self.fraction)?;
                    self.write("%:z")
                }
            },
        )
    }
}

/// Formats `timestamp` in `tz` with a chrono-compatible `format`.
///
/// # Errors
///
/// Returns an error if `format` is invalid.
pub fn format(
    timestamp: Timestamp,
    tz: &tz::TimeZone,
    format: &str,
) -> Result<String, FormatError> {
    let zoned = timestamp.to_zoned(tz.clone());
    let mut formatter = Formatter {
        fraction: auto_fraction(zoned.subsec_nanosecond()),
        seconds: timestamp_seconds(&timestamp),
        tm: BrokenDownTime::from(&zoned),
        out: String::new(),
    };
    formatter.write(format)?;
    Ok(formatter.out)
}

enum ParseItem {
    /// Format text to use instead of the specifier.
    Expand(&'static str),
    /// `%f`: up to 9 digits as an integer number of nanoseconds.
    Nanoseconds,
    Offset {
        zulu: bool,
        missing_minutes: bool,
    },
    /// `%Z`: chrono skips the name, since it cannot resolve abbreviations.
    SkipName,
    /// `%+`
    Rfc3339,
    /// `%y`: the year modulo 100.
    YearMod100,
    /// `%C`: the year divided by 100.
    YearDiv100,
}

fn classify_parse(spec: &Spec) -> Option<ParseItem> {
    if let Some(expansion) = spec.expansion(true) {
        return Some(ParseItem::Expand(expansion));
    }
    match spec.conversion {
        'f' if spec.is_plain() => Some(ParseItem::Nanoseconds),
        'z' if spec.flag.is_none() && !spec.width && !spec.dot && spec.colons <= 3 => {
            Some(ParseItem::Offset {
                zulu: false,
                missing_minutes: false,
            })
        }
        'z' if spec.flag == Some(b'#') && !spec.width && !spec.dot && spec.colons == 0 => {
            Some(ParseItem::Offset {
                zulu: true,
                missing_minutes: true,
            })
        }
        'Z' if spec.is_plain() => Some(ParseItem::SkipName),
        '+' if spec.is_plain() => Some(ParseItem::Rfc3339),
        'y' if spec.is_plain() => Some(ParseItem::YearMod100),
        'C' if spec.is_plain() => Some(ParseItem::YearDiv100),
        _ => None,
    }
}

struct Parser<'i> {
    tm: Option<BrokenDownTime>,
    rest: &'i str,
    year_mod_100: Option<i16>,
    year_div_100: Option<i16>,
}

impl<'i> Parser<'i> {
    fn run(&mut self, format: &str) -> Result<(), ParseError> {
        walk(
            format,
            classify_parse,
            || ParseError::new(BAD_FORMAT),
            |piece| match piece {
                Piece::Jiff(format) => self.segment(format),
                Piece::Special(ParseItem::Expand(format)) => self.run(format),
                Piece::Special(ParseItem::Nanoseconds) => self.nanoseconds(),
                Piece::Special(ParseItem::Offset {
                    zulu,
                    missing_minutes,
                }) => self.offset(zulu, missing_minutes),
                Piece::Special(ParseItem::SkipName) => {
                    self.rest = self.rest.trim_start_matches(|c: char| !c.is_whitespace());
                    Ok(())
                }
                Piece::Special(ParseItem::Rfc3339) => self.rfc3339(),
                Piece::Special(ParseItem::YearMod100) => {
                    let year = self.number(2)?;
                    check(self.year_mod_100, &year)?;
                    self.year_mod_100 = Some(year);
                    Ok(())
                }
                Piece::Special(ParseItem::YearDiv100) => {
                    let century = self.number(2)?;
                    check(self.year_div_100, &century)?;
                    self.year_div_100 = Some(century);
                    Ok(())
                }
            },
        )
    }

    fn segment(&mut self, format: &str) -> Result<(), ParseError> {
        if format.is_empty() {
            return Ok(());
        }
        let (tm, consumed) = BrokenDownTime::parse_prefix(format, self.rest)?;
        self.rest = self
            .rest
            .get(consumed..)
            .ok_or_else(|| ParseError::new(INVALID))?;
        if let Some(acc) = &mut self.tm {
            merge(acc, &tm)
        } else {
            self.tm = Some(tm);
            Ok(())
        }
    }

    fn tm(&mut self) -> &mut BrokenDownTime {
        self.tm.get_or_insert_with(BrokenDownTime::default)
    }

    /// Like chrono's numeric fields: skips leading whitespace, then takes up to `max_digits`.
    fn digits(&mut self, max_digits: usize) -> Result<&'i str, ParseError> {
        let s = self.rest.trim_start();
        let digits = s
            .bytes()
            .take(max_digits)
            .take_while(u8::is_ascii_digit)
            .count();
        if digits == 0 {
            return Err(ParseError::new(if s.is_empty() {
                TOO_SHORT
            } else {
                INVALID
            }));
        }
        self.rest = &s[digits..];
        Ok(&s[..digits])
    }

    fn number(&mut self, max_digits: usize) -> Result<i16, ParseError> {
        Ok(self
            .digits(max_digits)?
            .parse()
            .expect("at most 2 ASCII digits"))
    }

    fn nanoseconds(&mut self) -> Result<(), ParseError> {
        let nanosecond = self.digits(9)?.parse().expect("at most 9 ASCII digits");
        let tm = self.tm();
        check(tm.subsec_nanosecond(), &nanosecond)?;
        Ok(tm.set_subsec_nanosecond(Some(nanosecond))?)
    }

    fn offset(&mut self, zulu: bool, missing_minutes: bool) -> Result<(), ParseError> {
        let (rest, seconds) = scan_offset(self.rest.trim_start(), zulu, missing_minutes)?;
        self.rest = rest;
        self.set_offset(seconds)
    }

    fn set_offset(&mut self, seconds: i32) -> Result<(), ParseError> {
        let offset = Offset::from_seconds(seconds).map_err(|_| ParseError::new(OUT_OF_RANGE))?;
        let tm = self.tm();
        check(tm.offset(), &offset)?;
        tm.set_offset(Some(offset));
        Ok(())
    }

    /// chrono's relaxed RFC 3339 parser: spaces may separate components, the offset colon is
    /// optional, and `Z` or `UTC` stand for a zero offset.
    fn rfc3339(&mut self) -> Result<(), ParseError> {
        self.segment("%Y -%m -%d")?;
        self.rest = match self.rest.as_bytes().first() {
            Some(b't' | b'T' | b' ') => &self.rest[1..],
            Some(_) => return Err(ParseError::new(INVALID)),
            None => return Err(ParseError::new(TOO_SHORT)),
        };
        self.segment("%H :%M :%S%.f ")?;
        let s = self.rest.trim_start();
        if s.get(..3)
            .is_some_and(|utc| utc.eq_ignore_ascii_case("UTC"))
        {
            self.rest = &s[3..];
            return self.set_offset(0);
        }
        let (rest, seconds) = scan_offset(s, true, false)?;
        self.rest = rest;
        self.set_offset(seconds)
    }

    /// Sets the year from `%y`, and `%C` if present. Without a century, chrono maps `00..=69`
    /// to the 2000s and `70..=99` to the 1900s.
    fn finish_year(&mut self) -> Result<(), ParseError> {
        let Some(year_mod_100) = self.year_mod_100 else {
            return Ok(());
        };
        let year = match self.year_div_100 {
            Some(century) => century * 100 + year_mod_100,
            None if year_mod_100 < 70 => 2000 + year_mod_100,
            None => 1900 + year_mod_100,
        };
        let tm = self.tm();
        check(tm.year(), &year)?;
        Ok(tm.set_year(Some(year))?)
    }
}

/// Parses `input` with a chrono-compatible `format`.
///
/// Combine the result with [`to_timestamp`] or [`to_naive_datetime`], or with
/// [`BrokenDownTime`] accessors for partial values.
///
/// # Errors
///
/// Returns an error if `input` does not match `format`.
pub fn parse(input: &str, format: &str) -> Result<BrokenDownTime, ParseError> {
    let mut parser = Parser {
        tm: None,
        rest: input,
        year_mod_100: None,
        year_div_100: None,
    };
    parser.run(format)?;
    if !parser.rest.is_empty() {
        return Err(ParseError::new(TOO_LONG));
    }
    parser.finish_year()?;
    Ok(parser.tm.unwrap_or_default())
}

/// chrono's offset scanner: `[+-]HH[[:]MM]`, also accepting `−` (U+2212) and any run of colons
/// or whitespace as the separator.
fn scan_offset(s: &str, zulu: bool, missing_minutes: bool) -> Result<(&str, i32), ParseError> {
    if zulu && matches!(s.as_bytes().first(), Some(b'Z' | b'z')) {
        return Ok((&s[1..], 0));
    }
    let (negative, s) = if let Some(s) = s.strip_prefix('+') {
        (false, s)
    } else if let Some(s) = s.strip_prefix(['-', '−']) {
        (true, s)
    } else if s.is_empty() {
        return Err(ParseError::new(TOO_SHORT));
    } else {
        return Err(ParseError::new(INVALID));
    };

    let two_digits = |s: &str| match s.as_bytes() {
        [a, b, ..] if a.is_ascii_digit() && b.is_ascii_digit() => {
            Some(i32::from((a - b'0') * 10 + (b - b'0')))
        }
        _ => None,
    };
    let hours = match two_digits(s) {
        Some(hours) => hours,
        None if s.len() < 2 => return Err(ParseError::new(TOO_SHORT)),
        None => return Err(ParseError::new(INVALID)),
    };
    let s = s[2..].trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    let (s, minutes) = match two_digits(s) {
        Some(minutes) if minutes < 60 => (&s[2..], minutes),
        Some(_) => return Err(ParseError::new(OUT_OF_RANGE)),
        None if missing_minutes => (s, 0),
        None if s.len() < 2 => return Err(ParseError::new(TOO_SHORT)),
        None => return Err(ParseError::new(INVALID)),
    };
    let seconds = hours * 3600 + minutes * 60;
    Ok((s, if negative { -seconds } else { seconds }))
}

fn check<T: PartialEq>(existing: Option<T>, new: &T) -> Result<(), ParseError> {
    if existing.is_some_and(|existing| existing != *new) {
        Err(ParseError::new(IMPOSSIBLE))
    } else {
        Ok(())
    }
}

/// Copies the fields `src` has into `dst`, failing if both have a field with different values.
fn merge(dst: &mut BrokenDownTime, src: &BrokenDownTime) -> Result<(), ParseError> {
    macro_rules! merge {
        ($get:ident, $set:ident) => {
            if let Some(value) = src.$get() {
                check(dst.$get(), &value)?;
                dst.$set(Some(value))?;
            }
        };
        ($get:ident, $set:ident, infallible) => {
            if let Some(value) = src.$get() {
                check(dst.$get(), &value)?;
                dst.$set(Some(value));
            }
        };
    }

    // The meridiem adjusts hours, so it is set first; adjusting is idempotent.
    merge!(meridiem, set_meridiem, infallible);
    merge!(year, set_year);
    merge!(month, set_month);
    merge!(day, set_day);
    merge!(day_of_year, set_day_of_year);
    merge!(iso_week_year, set_iso_week_year);
    merge!(iso_week, set_iso_week);
    merge!(sunday_based_week, set_sunday_based_week);
    merge!(monday_based_week, set_monday_based_week);
    merge!(hour, set_hour);
    merge!(minute, set_minute);
    merge!(second, set_second);
    merge!(subsec_nanosecond, set_subsec_nanosecond);
    merge!(offset, set_offset, infallible);
    merge!(weekday, set_weekday, infallible);
    merge!(timestamp, set_timestamp, infallible);
    if let Some(name) = src.iana_time_zone() {
        check(dst.iana_time_zone(), &name)?;
        dst.set_iana_time_zone(Some(name.to_owned()));
    }
    Ok(())
}

/// The timestamp from `%s`, plus any parsed fraction.
fn explicit_timestamp(tm: &BrokenDownTime) -> Option<Result<Timestamp, ParseError>> {
    let timestamp = tm.timestamp()?;
    let nanosecond = SignedDuration::from_nanos(i64::from(tm.subsec_nanosecond().unwrap_or(0)));
    Some(
        timestamp
            .checked_add(nanosecond)
            .map_err(|_| ParseError::new(OUT_OF_RANGE)),
    )
}

/// The parsed civil date and time. Like chrono, the hour and minute are required.
fn to_datetime(tm: &BrokenDownTime) -> Result<civil::DateTime, ParseError> {
    if tm.hour().is_none() || tm.minute().is_none() {
        return Err(ParseError::new(NOT_ENOUGH));
    }
    Ok(tm.to_date()?.to_datetime(tm.to_time()?))
}

/// Resolves parsed fields to a civil date and time, ignoring any offset.
///
/// # Errors
///
/// Returns an error if the date or time is missing or invalid.
pub fn to_naive_datetime(tm: &BrokenDownTime) -> Result<civil::DateTime, ParseError> {
    match explicit_timestamp(tm) {
        Some(timestamp) => Ok(Offset::UTC.to_datetime(timestamp?)),
        None => to_datetime(tm),
    }
}

/// Resolves parsed fields to a timestamp, interpreting values without an offset in `tz`.
///
/// A parsed offset must match the offset of `tz` at that time. It also picks the instant when
/// the local time is ambiguous in `tz`; without it, ambiguous and non-existent local times are
/// errors.
///
/// # Errors
///
/// Returns an error if the fields do not form a unique instant.
pub fn to_timestamp(tm: &BrokenDownTime, tz: &tz::TimeZone) -> Result<Timestamp, ParseError> {
    if let Some(timestamp) = explicit_timestamp(tm) {
        return timestamp;
    }
    let datetime = to_datetime(tm)?;
    let matches = |offset: Offset| tm.offset().is_none_or(|parsed| parsed == offset);
    let offset = match tz.to_ambiguous_timestamp(datetime).offset() {
        AmbiguousOffset::Unambiguous { offset } if matches(offset) => offset,
        AmbiguousOffset::Fold { before, after } => match (matches(before), matches(after)) {
            (true, false) => before,
            (false, true) => after,
            (true, true) => return Err(ParseError::new(NOT_ENOUGH)),
            (false, false) => return Err(ParseError::new(IMPOSSIBLE)),
        },
        AmbiguousOffset::Unambiguous { .. } | AmbiguousOffset::Gap { .. } => {
            return Err(ParseError::new(IMPOSSIBLE));
        }
    };
    offset
        .to_timestamp(datetime)
        .map_err(|_| ParseError::new(OUT_OF_RANGE))
}

/// Resolves parsed fields that include an offset (or a `%s` timestamp) to a timestamp.
///
/// # Errors
///
/// Returns an error if the offset, date or time is missing or invalid.
pub fn to_offset_timestamp(tm: &BrokenDownTime) -> Result<Timestamp, ParseError> {
    if let Some(timestamp) = explicit_timestamp(tm) {
        return timestamp;
    }
    let offset = tm.offset().ok_or_else(|| ParseError::new(NOT_ENOUGH))?;
    offset
        .to_timestamp(to_datetime(tm)?)
        .map_err(|_| ParseError::new(OUT_OF_RANGE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn formats_chrono_specifiers() {
        let ts = utc("2001-07-08T00:34:59.026490Z");
        let tz = tz::TimeZone::get("Asia/Kolkata").unwrap();
        let cases = [
            ("%f", "026490000"),
            ("%.f", ".026490"),
            ("%+", "2001-07-08T06:04:59.026490+05:30"),
            ("%v", " 8-Jul-2001"),
            ("%c", "Sun Jul  8 06:04:59 2001"),
            ("%x %X %r", "07/08/01 06:04:59 06:04:59 AM"),
            ("%Y-%m-%d %H:%M:%S%.3f %z", "2001-07-08 06:04:59.026 +0530"),
            ("100%% %Z", "100% IST"),
        ];
        for (format, want) in cases {
            assert_eq!(super::format(ts, &tz, format).unwrap(), want, "{format}");
        }
        let whole = utc("2001-07-08T00:34:59Z");
        assert_eq!(
            super::format(whole, &tz::TimeZone::UTC, "%+").unwrap(),
            "2001-07-08T00:34:59+00:00"
        );
        // Like chrono, `%s` rounds down instead of towards zero.
        let before_epoch = utc("1969-12-31T23:59:58.5Z");
        assert_eq!(
            super::format(before_epoch, &tz::TimeZone::UTC, "%s").unwrap(),
            "-2"
        );
        assert!(super::format(ts, &tz::TimeZone::UTC, "%").is_err());
    }

    #[test]
    fn parses_chrono_specifiers() {
        let offset = |input: &str, format: &str| to_offset_timestamp(&parse(input, format)?);
        let want = Ok(utc("2001-07-08T00:34:59Z"));
        assert_eq!(offset("2001-07-08 06:04:59 +05:30", "%F %T %z"), want);
        assert_eq!(offset("2001-07-08 06:04:59 +0530", "%F %T %:z"), want);
        assert_eq!(
            offset("2001-07-08 06:04:59 +05", "%F %T %#z"),
            Ok(utc("2001-07-08T01:04:59Z"))
        );
        assert_eq!(offset("2001-07-08 00:34:59 Z", "%F %T %#z"), want);
        assert_eq!(offset("2001-07-08t00:34:59Z", "%+"), want);
        assert_eq!(offset("2001-07-08T06:04:59+05:30", "%+"), want);
        assert_eq!(offset("2001-07-08 00:34:59 UTC", "%+"), want);
        assert_eq!(offset("Sun Jul  8 00:34:59 2001 +0000", "%c %z"), want);
        assert_eq!(
            offset("994552499.5", "%s%.f"),
            Ok(utc("2001-07-08T00:34:59.5Z"))
        );
        assert!(offset("2001-07-08 00:34:59", "%F %T").is_err());
        assert!(offset("2001-07-08 00:34:59 +0000 extra", "%F %T %z").is_err());

        let local = |input: &str, format: &str| {
            to_timestamp(
                &parse(input, format)?,
                &tz::TimeZone::get("Europe/Paris").unwrap(),
            )
        };
        // `%f` holds integer nanoseconds and `%Z` is skipped.
        assert_eq!(
            local("2001-07-08 02:34:59 123 CEST", "%F %T %f %Z"),
            Ok(utc("2001-07-08T00:34:59.000000123Z"))
        );
        // A parsed offset must agree with the time zone.
        assert!(local("2001-07-08 02:34:59 +0000", "%F %T %z").is_err());
        // An offset picks one side of a fold; without it the time is ambiguous.
        assert_eq!(
            local("2023-10-29 02:30:00 +0100", "%F %T %z"),
            Ok(utc("2023-10-29T01:30:00Z"))
        );
        assert!(local("2023-10-29 02:30:00", "%F %T").is_err());
        // Times in a gap do not exist.
        assert!(local("2023-03-26 02:30:00", "%F %T").is_err());
        // Like chrono, a time needs at least the hour and minute.
        assert!(local("2001-07-08", "%F").is_err());
        assert!(local("2001-07-08 02", "%F %H").is_err());
        // Without a century, chrono maps `%y` values below 70 to the 2000s.
        assert_eq!(
            offset("07/08/69 00:00 +0000", "%D %R %z"),
            Ok(utc("2069-07-08T00:00:00Z"))
        );
        assert_eq!(
            offset("07/08/70 00:00 +0000", "%x %R %z"),
            Ok(utc("1970-07-08T00:00:00Z"))
        );
        assert_eq!(
            offset("19 69-07-08 00:00 +0000", "%C %y-%m-%d %R %z"),
            Ok(utc("1969-07-08T00:00:00Z"))
        );
    }
}
