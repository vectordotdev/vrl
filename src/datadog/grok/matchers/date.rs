use std::fmt::Formatter;

use crate::value::Value;
use chrono::{
    DateTime, Datelike, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc,
};
use chrono_tz::{Tz, UTC};
use peeking_take_while::PeekableExt;
use regex::Regex;
use tracing::warn;

use super::super::parse_grok::Error as GrokRuntimeError;

/// converts Joda time format to strptime format
pub fn convert_time_format(format: &str) -> Result<String, String> {
    let mut time_format = String::new();
    let mut chars = format.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_uppercase() || c.is_ascii_lowercase() {
            let token: String = chars.by_ref().peeking_take_while(|&cn| cn == c).collect();
            match token.chars().next().unwrap() {
                // hour of day (number, 1..12)
                'h' => time_format.push_str("%I"),
                // hour of day (number, 0..23)
                'H' => time_format.push_str("%H"),
                //  minute of hour
                'm' => time_format.push_str("%M"),
                // second of minute
                's' => time_format.push_str("%S"),
                // fraction of second
                'S' => {
                    time_format.pop(); // drop the fraction charactor(e.g. . or , )
                    time_format.push_str("%.f"); // Decimal fraction of a second. Consumes the leading dot.
                }
                // year
                'y' | 'Y' if token.len() == 2 => time_format.push_str("%y"),
                'y' | 'Y' => time_format.push_str("%Y"),
                // weekyear
                'x' => time_format.push_str("%D"),
                // century
                'c' | 'C' => time_format.push_str("%C"),
                // day w/o 0-padding
                'd' if token.len() == 1 => time_format.push_str("%-d"),
                // day with 0-padding
                'd' => time_format.push_str("%d"),
                // day of week
                'e' => time_format.push_str("%u"),
                // day of year
                'D' => time_format.push_str("%j"),
                // week of year
                'w' => time_format.push_str("%V"),
                // month of year
                'M' => {
                    if token.len() == 1 {
                        // Month number w/o 0-padding
                        time_format.push_str("%-m");
                    } else if token.len() == 2 {
                        // Month number with 0-padding
                        time_format.push_str("%m");
                    } else if token.len() == 3 {
                        // Abbreviated month name. Always 3 letters.
                        time_format.push_str("%b");
                    } else if token.len() > 3 {
                        // Full month name
                        time_format.push_str("%B");
                    }
                }
                // AM/PM
                'a' => time_format.push_str("%p"),
                // dayOfWeek (text)
                'E' if token.len() == 3 => time_format.push_str("%a"),
                'E' if token.len() > 3 => time_format.push_str("%A"),
                // time zone (text)
                'z' => time_format.push_str("%Z"),
                // time zone offset
                'Z' => {
                    if token.len() == 1 {
                        time_format.push_str("%z");
                    } else if token.len() == 2 {
                        time_format.push_str("%:z");
                    }
                }
                _ => return Err(format!("invalid date format '{}'", format)),
            }
        } else if c == '\''
        // quoted literal
        {
            let literal: String = chars
                .by_ref()
                .skip(1)
                .take_while(|&cn| cn != '\'')
                .collect();
            time_format.push_str(literal.as_str());
        } else {
            time_format.push(chars.next().unwrap());
        }
    }
    Ok(time_format)
}

pub struct RegexResult {
    pub regex: String,
    pub with_tz: bool,
}

pub fn parse_timezone(tz: &str) -> Result<FixedOffset, String> {
    let tz = match tz {
        "GMT" | "UTC" | "UT" | "Z" => FixedOffset::east_opt(0).expect("invalid timestamp"),
        _ if tz.starts_with('+') || tz.starts_with('-') => parse_offset(tz)?,
        _ if tz.contains('+') => parse_offset(&tz[tz.find('+').unwrap()..])?,
        _ if tz.contains('-') => parse_offset(&tz[tz.find('-').unwrap()..])?,
        tz => parse_tz_id_or_name(tz)?,
    };
    Ok(tz)
}

fn parse_tz_id_or_name(tz: &str) -> Result<FixedOffset, String> {
    let tz = tz.parse::<Tz>().map_err(|e| e.to_string())?;
    Ok(Utc::now().with_timezone(&tz).offset().fix())
}

fn parse_offset(tz: &str) -> Result<FixedOffset, String> {
    if tz.len() <= 3 {
        // +5, -12
        let hours_diff = tz.parse::<i32>().map_err(|e| e.to_string())?;
        return Ok(FixedOffset::east_opt(hours_diff * 3600).expect("invalid timestamp"));
    }
    let offset_format = if tz.contains(':') { "%:z" } else { "%z" };
    // apparently the easiest way to parse tz offset is parsing the complete datetime
    let date_str = format!("2020-04-12 22:10:57 {}", tz);
    let datetime =
        DateTime::parse_from_str(&date_str, &format!("%Y-%m-%d %H:%M:%S {}", offset_format))
            .map_err(|e| e.to_string())?;
    Ok(datetime.timezone())
}

const FRACTION_CHAR_GROUP: &str = "fr";

pub fn time_format_to_regex(format: &str, with_captures: bool) -> Result<RegexResult, String> {
    let mut regex = String::new();
    let mut chars = format.chars().peekable();
    let mut with_tz = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_uppercase() || c.is_ascii_lowercase() {
            let token: String = chars.by_ref().peeking_take_while(|&cn| cn == c).collect();
            match c {
                'h' | 'H' | 'm' | 's' | 'Y' | 'x' | 'c' | 'C' | 'e' | 'D' | 'w' => {
                    regex.push_str(format!("[\\d]{{{}}}", token.len()).as_str())
                }
                // days
                'd' if token.len() == 1 => regex.push_str("[\\d]{1,2}"), // support 0-padding
                'd' => regex.push_str(format!("[\\d]{{{}}}", token.len()).as_str()),
                // years
                'y' if token.len() == 1 => regex.push_str("[\\d]{4}"), // expand y to yyyy
                'y' => regex.push_str(format!("[\\d]{{{}}}", token.len()).as_str()),
                // decimal fraction of a second
                'S' => {
                    if let Some(fraction_char) = regex.pop() {
                        let fraction_char = if fraction_char == '.' {
                            regex.pop(); // drop the escape character for .
                            "\\.".to_string() // escape . in regex
                        } else {
                            fraction_char.to_string()
                        };
                        if with_captures {
                            // add the non-capturing group for the fraction of a second so we can convert value to a dot-leading format later
                            regex.push_str(
                                format!("(?P<{}>{})", FRACTION_CHAR_GROUP, fraction_char).as_str(),
                            );
                        } else {
                            regex.push_str(&fraction_char);
                        }
                    }
                    regex.push_str(&format!("[\\d]{{{}}}", token.len()));
                }
                // Month number
                'M' if token.len() == 1 => regex.push_str("[\\d]{1,2}"), // with 0-padding
                'M' if token.len() == 2 => regex.push_str("[\\d]{2}"),
                'M' if token.len() == 3 =>
                // Abbreviated month name. Always 3 letters.
                {
                    regex.push_str("[\\w]{3}")
                }
                'M' if token.len() > 3 =>
                // Full month name
                {
                    regex.push_str("[\\w]+")
                }
                // AM/PM
                'a' => regex.push_str("(?:[aA][mM]|[pP][mM])"),
                // dayOfWeek (text)
                'E' if token.len() == 3 =>
                // Abbreviated day name. Always 3 letters.
                {
                    regex.push_str("[\\w]{3}")
                }
                'E' if token.len() > 3 => regex.push_str("[\\w]+"),
                // time zone (text)
                'z' => {
                    if token.len() >= 4 {
                        if with_captures {
                            regex.push_str("(?P<tz>[\\w]+(?:/[\\w]+)?)");
                        } else {
                            regex.push_str("[\\w]+(?:\\/[\\w]+)?");
                        }
                    } else if with_captures {
                        regex.push_str("(?P<tz>[\\w]+)");
                    } else {
                        regex.push_str("[\\w]+");
                    }
                    with_tz = true;
                }
                // time zone offset
                'Z' => {
                    if token.len() == 1 || token.len() == 2 {
                        regex.push_str("(?:Z|[+-]\\d\\d:?\\d\\d)");
                    } else {
                        regex.push_str("[\\w]+(?:/[\\w]+)?");
                    }
                    with_tz = true;
                }
                _ => return Err(format!("invalid date format '{}'", format)),
            }
        } else if c == '\'' {
            // quoted literal
            {
                let literal: String = chars
                    .by_ref()
                    .skip(1)
                    .take_while(|&cn| cn != '\'')
                    .collect();
                regex.push_str(literal.as_str());
            }
        } else {
            if c == '.' {
                regex.push('\\'); // escape . in regex
            }
            regex.push(c);
            chars.next();
        }
    }
    Ok(RegexResult { regex, with_tz })
}

pub fn apply_date_filter(value: &Value, filter: &DateFilter) -> Result<Value, GrokRuntimeError> {
    let original_value = String::from_utf8_lossy(value.as_bytes().ok_or_else(|| {
        GrokRuntimeError::FailedToApplyFilter(filter.to_string(), value.to_string())
    })?);
    let (strp_format, mut datetime) =
        adjust_strp_format_and_value(&filter.strp_format, &original_value);

    // Ideally this Z should be quoted in the pattern, but DataDog supports this as a special case:
    // yyyy-MM-dd'T'HH:mm:ss.SSSZ - e.g. 2016-09-02T15:02:29.648Z
    if datetime.ends_with('Z') && filter.original_format.ends_with('Z') {
        datetime.pop(); // drop Z
        datetime.push_str("+0000");
    };

    if let Some(tz) = filter
        .regex
        .captures(&original_value)
        .and_then(|caps| caps.name("tz"))
    {
        let tz = tz.as_str();
        let tz: Tz = tz.parse().map_err(|error| {
            warn!(message = "Error parsing tz", tz = %tz, % error);
            GrokRuntimeError::FailedToApplyFilter(filter.to_string(), original_value.to_string())
        })?;
        replace_sec_fraction_with_dot(filter, &mut datetime);
        let naive_date = NaiveDateTime::parse_from_str(&datetime, &strp_format).map_err(|error|
        {
            warn!(message = "Error parsing date", value = %original_value, format = %strp_format, % error);
            GrokRuntimeError::FailedToApplyFilter(
                filter.to_string(),
                original_value.to_string(),
            )
        })?;
        let dt = tz
            .from_local_datetime(&naive_date)
            .single()
            .ok_or_else(|| {
                GrokRuntimeError::FailedToApplyFilter(
                    filter.to_string(),
                    original_value.to_string(),
                )
            })?;
        Ok(Value::from(
            Utc.from_utc_datetime(&dt.naive_utc()).timestamp_millis(),
        ))
    } else {
        replace_sec_fraction_with_dot(filter, &mut datetime);
        if filter.tz_aware {
            // parse as a tz-aware complete date/time
            let timestamp = DateTime::parse_from_str(&datetime, &strp_format).map_err(|error| {
                warn!(message = "Error parsing date", date = %original_value, % error);
                GrokRuntimeError::FailedToApplyFilter(
                    filter.to_string(),
                    original_value.to_string(),
                )
            })?;
            Ok(Value::from(timestamp.to_utc().timestamp_millis()))
        } else if let Ok(dt) = NaiveDateTime::parse_from_str(&datetime, &strp_format) {
            // try parsing as a naive datetime
            if let Some(tz) = &filter.target_tz {
                let tzs = parse_timezone(tz).map_err(|error| {
                    warn!(message = "Error parsing tz", tz = %tz, % error);
                    GrokRuntimeError::FailedToApplyFilter(
                        filter.to_string(),
                        original_value.to_string(),
                    )
                })?;
                let dt = tzs.from_local_datetime(&dt).single().ok_or_else(|| {
                    warn!(message = "Error parsing date", date = %original_value);
                    GrokRuntimeError::FailedToApplyFilter(
                        filter.to_string(),
                        original_value.to_string(),
                    )
                })?;
                Ok(Value::from(dt.to_utc().timestamp_millis()))
            } else {
                Ok(Value::from(dt.and_utc().timestamp_millis()))
            }
        } else if let Ok(nt) = NaiveTime::parse_from_str(&datetime, &strp_format) {
            // try parsing as a naive time
            Ok(Value::from(
                NaiveDateTime::new(
                    NaiveDate::from_ymd_opt(1970, 1, 1).expect("invalid date"),
                    nt,
                )
                .and_utc()
                .timestamp_millis(),
            ))
        } else {
            // try parsing as a naive date
            let nd = NaiveDate::parse_from_str(&datetime, &strp_format).map_err(|error| {
                warn!(message = "Error parsing date", date = %original_value, % error);
                GrokRuntimeError::FailedToApplyFilter(
                    filter.to_string(),
                    original_value.to_string(),
                )
            })?;
            let datetime_tz = UTC
                .from_local_datetime(&NaiveDateTime::new(
                    nd,
                    NaiveTime::from_hms_opt(0, 0, 0).expect("invalid timestamp"),
                ))
                .single()
                .ok_or_else(|| {
                    warn!(message = "Error parsing date", date = %original_value);
                    GrokRuntimeError::FailedToApplyFilter(
                        filter.to_string(),
                        original_value.to_string(),
                    )
                })?;
            Ok(Value::from(
                Utc.from_utc_datetime(&datetime_tz.naive_utc())
                    .timestamp_millis(),
            ))
        }
    }
}

/// adjusts strp format and value to matches formats w/o the date or the year
pub fn adjust_strp_format_and_value(strp_format: &str, original_value: &str) -> (String, String) {
    let mut adjusted_format = String::from(strp_format);
    let mut adjusted_value = String::from(original_value);
    let now = Utc::now();

    // day is missing
    if !strp_format.contains('d') {
        adjusted_format = format!("%-m %-d {}", adjusted_format);
        adjusted_value = format!("{} {} {}", now.month(), now.day(), adjusted_value);
    }
    // year is missing
    if !strp_format.contains('y') && !strp_format.contains('Y') {
        adjusted_format = format!("%Y {}", adjusted_format);
        adjusted_value = format!("{} {}", now.year(), adjusted_value);
    }

    (adjusted_format, adjusted_value)
}

/// Replace fraction of a second char with a dot - we always use %.f in strptime format
fn replace_sec_fraction_with_dot(filter: &DateFilter, value: &mut String) {
    if let Some(caps) = filter.regex.captures(value) {
        if let Some(m) = caps.name(FRACTION_CHAR_GROUP) {
            value.replace_range(m.start()..m.end(), ".");
        }
    }
}

#[derive(Debug, Clone)]
pub struct DateFilter {
    // an original date format used for debugging purposes
    pub original_format: String,
    // strp time format used to parse the date
    pub strp_format: String,
    // whether the format is naive or timezone-aware
    pub tz_aware: bool,
    // an regex, used to extract timezone or a fraction of a second char
    pub regex: Regex,
    // an optional target TZ name
    pub target_tz: Option<String>,
}

impl std::fmt::Display for DateFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "date(\"{}\")", self.original_format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjusts_datetime_format_and_value_when_day_missing() {
        let (adj_format, adj_value) = adjust_strp_format_and_value("%H:%M:%S", "12:03:42");
        let now = Utc::now();
        let expected_datetime = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day())
            .unwrap()
            .and_hms_opt(12, 3, 42)
            .unwrap();
        // make sure we can parse the date with the expected result
        assert_eq!(
            expected_datetime,
            NaiveDateTime::parse_from_str(&adj_value, &adj_format).unwrap()
        )
    }

    #[test]
    fn adjusts_datetime_format_and_value_when_year_missing() {
        let (adj_format, adj_value) =
            adjust_strp_format_and_value("%-d/%-m %H:%M:%S", "25/03 12:03:42");
        let now = Utc::now();
        let expected_datetime = NaiveDate::from_ymd_opt(now.year(), 3, 25)
            .unwrap()
            .and_hms_opt(12, 3, 42)
            .unwrap();
        // make sure we can parse the date with the expected result
        assert_eq!(
            expected_datetime,
            NaiveDateTime::parse_from_str(&adj_value, &adj_format).unwrap()
        )
    }
}
