use crate::compiler::datetime;
use crate::compiler::prelude::*;
use chrono::{Datelike, FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, Timelike};
use jiff::{
    SignedDuration, Timestamp, civil,
    tz::{self, AmbiguousOffset},
};
use std::collections::BTreeMap;
use syslog_loose::{IncompleteDate, Message, ProcId, Protocol, Variant};

pub(crate) fn parse_syslog(value: &Value, ctx: &Context) -> Resolved {
    let message = value.try_bytes_utf8_lossy()?;
    // Without a time zone, `syslog_loose` uses the system time zone.
    let timezone = match ctx.timezone() {
        TimeZone::Local => None,
        TimeZone::Named(tz) => Some(ChronoTz(tz)),
    };
    let parsed = syslog_loose::parse_message_with_year_exact_tz(
        &message,
        resolve_year,
        timezone,
        Variant::Either,
    )?;
    Ok(message_to_value(parsed))
}

/// Adapts a `jiff` time zone to the `chrono` trait that `syslog_loose` requires.
#[derive(Clone, Copy, Debug)]
struct ChronoTz<'a>(&'a tz::TimeZone);

#[derive(Clone, Copy, Debug)]
struct ChronoOffset<'a> {
    tz: ChronoTz<'a>,
    fixed: FixedOffset,
}

impl chrono::Offset for ChronoOffset<'_> {
    fn fix(&self) -> FixedOffset {
        self.fixed
    }
}

impl<'a> ChronoTz<'a> {
    fn offset(self, offset: tz::Offset) -> ChronoOffset<'a> {
        ChronoOffset {
            tz: self,
            fixed: FixedOffset::east_opt(offset.seconds()).expect("offsets are less than a day"),
        }
    }
}

fn to_civil(dt: &NaiveDateTime) -> Option<civil::DateTime> {
    civil::DateTime::new(
        i16::try_from(dt.year()).ok()?,
        i8::try_from(dt.month()).ok()?,
        i8::try_from(dt.day()).ok()?,
        i8::try_from(dt.hour()).ok()?,
        i8::try_from(dt.minute()).ok()?,
        i8::try_from(dt.second()).ok()?,
        0,
    )
    .ok()
}

impl<'a> chrono::TimeZone for ChronoTz<'a> {
    type Offset = ChronoOffset<'a>;

    fn from_offset(offset: &Self::Offset) -> Self {
        offset.tz
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<Self::Offset> {
        self.offset_from_local_datetime(&local.and_time(chrono::NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<Self::Offset> {
        let Some(local) = to_civil(local) else {
            return MappedLocalTime::None;
        };
        match self.0.to_ambiguous_timestamp(local).offset() {
            AmbiguousOffset::Unambiguous { offset } => MappedLocalTime::Single(self.offset(offset)),
            AmbiguousOffset::Gap { .. } => MappedLocalTime::None,
            AmbiguousOffset::Fold { before, after } => {
                MappedLocalTime::Ambiguous(self.offset(before), self.offset(after))
            }
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> Self::Offset {
        self.offset_from_utc_datetime(&utc.and_time(chrono::NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> Self::Offset {
        let offset = to_civil(utc)
            .and_then(|utc| tz::Offset::UTC.to_timestamp(utc).ok())
            .map_or(tz::Offset::UTC, |timestamp| self.0.to_offset(timestamp));
        self.offset(offset)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseSyslog;

impl Function for ParseSyslog {
    fn identifier(&self) -> &'static str {
        "parse_syslog"
    }

    fn description(&self) -> &'static str {
        "Parses the `value` in [Syslog](https://en.wikipedia.org/wiki/Syslog) format."
    }

    fn category(&self) -> &'static str {
        Category::Parse.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a properly formatted Syslog message."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn notices(&self) -> &'static [&'static str] {
        &[
            indoc! {"
                The function makes a best effort to parse the various Syslog formats that exists out
                in the wild. This includes [RFC 6587](https://tools.ietf.org/html/rfc6587),
                [RFC 5424](https://tools.ietf.org/html/rfc5424),
                [RFC 3164](https://tools.ietf.org/html/rfc3164), and other common variations (such
                as the Nginx Syslog style).
            "},
            "All values are returned as strings. We recommend manually coercing values to desired types as you see fit.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::BYTES,
            "The text containing the Syslog message to parse.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Parse Syslog log (5424)",
            source: r#"parse_syslog!(s'<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut="3" eventSource= "Application" eventID="1011"] Try to override the THX port, maybe it will reboot the neural interface!')"#,
            result: Ok(indoc! {r#"{
                "appname": "non",
                "exampleSDID@32473": {
                    "eventID": "1011",
                    "eventSource": "Application",
                    "iut": "3"
                },
                "facility": "user",
                "hostname": "dynamicwireless.name",
                "message": "Try to override the THX port, maybe it will reboot the neural interface!",
                "msgid": "ID931",
                "procid": 2426,
                "severity": "notice",
                "timestamp": "2020-03-13T20:45:38.119Z",
                "version": 1
            }"#}),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ParseSyslogFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ParseSyslogFn {
    pub(crate) value: Box<dyn Expression>,
}

impl FunctionExpression for ParseSyslogFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        parse_syslog(&value, ctx)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).fallible()
    }
}

/// Function used to resolve the year for syslog messages that don't include the
/// year. If the current month is January, and the syslog message is for
/// December, it will take the previous year. Otherwise, take the current year.
fn resolve_year((month, _date, _hour, _min, _sec): IncompleteDate) -> i32 {
    let now = datetime::utc_now();
    if now.month() == 1 && month == 12 {
        i32::from(now.year()) - 1
    } else {
        i32::from(now.year())
    }
}

/// Create a `Value::Map` from the fields of the given syslog message.
fn message_to_value(message: Message<&str>) -> Value {
    let mut result = ObjectMap::new();

    result.insert("message".into(), message.msg.into());

    if let Some(host) = message.hostname {
        result.insert("hostname".into(), host.into());
    }

    if let Some(severity) = message.severity {
        result.insert("severity".into(), severity.as_str().into());
    }

    if let Some(facility) = message.facility {
        result.insert("facility".into(), facility.as_str().into());
    }

    if let Protocol::RFC5424(version) = message.protocol {
        result.insert("version".into(), version.into());
    }

    if let Some(app_name) = message.appname {
        result.insert("appname".into(), app_name.into());
    }

    if let Some(msg_id) = message.msgid {
        result.insert("msgid".into(), msg_id.into());
    }

    if let Some(timestamp) = message.timestamp {
        let nanosecond = SignedDuration::from_nanos(i64::from(timestamp.timestamp_subsec_nanos()));
        if let Ok(timestamp) =
            Timestamp::from_second(timestamp.timestamp()).and_then(|ts| ts.checked_add(nanosecond))
        {
            result.insert("timestamp".into(), timestamp.into());
        }
    }

    if let Some(procid) = message.procid {
        let value: Value = match procid {
            ProcId::PID(pid) => pid.into(),
            ProcId::Name(name) => name.to_string().into(),
        };
        result.insert("procid".into(), value);
    }

    for element in message.structured_data {
        let mut sdata = ObjectMap::new();
        for (name, value) in element.params() {
            sdata.insert((*name).into(), value.into());
        }
        result.insert(element.id.into(), sdata.into());
    }

    result.into()
}

fn inner_kind() -> BTreeMap<Field, Kind> {
    BTreeMap::from([
        ("message".into(), Kind::bytes()),
        ("hostname".into(), Kind::bytes().or_null()),
        ("severity".into(), Kind::bytes().or_null()),
        ("facility".into(), Kind::bytes().or_null()),
        ("appname".into(), Kind::bytes().or_null()),
        ("msgid".into(), Kind::bytes().or_null()),
        ("timestamp".into(), Kind::timestamp().or_null()),
        ("procid".into(), Kind::bytes().or_integer().or_null()),
        ("version".into(), Kind::integer().or_null()),
    ])
}

#[cfg(test)]
mod tests {
    use crate::btreemap;

    use super::*;

    test_function![
        parse_syslog => ParseSyslog;

        valid {
            args: func_args![value: r#"<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut="3" eventSource= "Application" eventID="1011"] Try to override the THX port, maybe it will reboot the neural interface!"#],
            want: Ok(btreemap! {
                "severity" => "notice",
                "facility" => "user",
                "timestamp" => jiff::civil::datetime(2020, 3, 13, 20, 45, 38, 119_000_000).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "hostname" => "dynamicwireless.name",
                "appname" => "non",
                "procid" => 2426,
                "msgid" => "ID931",
                "exampleSDID@32473" => btreemap! {
                    "iut" => "3",
                    "eventSource" => "Application",
                    "eventID" => "1011",
                },
                "message" => "Try to override the THX port, maybe it will reboot the neural interface!",
                "version" => 1,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        invalid {
            args: func_args![value: "not much of a syslog message"],
            want: Err("unable to parse input as valid syslog message".to_string()),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        haproxy {
            args: func_args![value: "<133>Jun 13 16:33:35 haproxy[73411]: Proxy sticky-servers started."],
            want: Ok(btreemap! {
                    "facility" => "local0",
                    "severity" => "notice",
                    "message" => "Proxy sticky-servers started.",
                    "timestamp" => jiff::civil::datetime(datetime::utc_now().year(), 6, 13, 16, 33, 35, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                    "appname" => "haproxy",
                    "procid" => 73411,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        missing_pri {
            args: func_args![value: "Jun 13 16:33:35 haproxy[73411]: I am missing a pri."],
            want: Ok(btreemap! {
                "message" => "I am missing a pri.",
                "timestamp" => jiff::civil::datetime(datetime::utc_now().year(), 6, 13, 16, 33, 35, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "appname" => "haproxy",
                "procid" => 73411,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        empty_sd_element {
            args: func_args![value: "<13>1 2019-02-13T19:48:34+00:00 74794bfb6795 root 8449 - [empty] qwerty"],
            want: Ok(btreemap!{
                "message" => "qwerty",
                "appname" => "root",
                "facility" => "user",
                "hostname" => "74794bfb6795",
                "message" => "qwerty",
                "procid" => 8449,
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2019, 2, 13, 19, 48, 34, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1,
                "empty" => btreemap! {},
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        non_empty_sd_element {
            args: func_args![value: r#"<13>1 2019-02-13T19:48:34+00:00 74794bfb6795 root 8449 - [non_empty x="1"][empty] qwerty"#],
            want: Ok(btreemap!{
                "message" => "qwerty",
                "appname" => "root",
                "facility" => "user",
                "hostname" => "74794bfb6795",
                "message" => "qwerty",
                "procid" => 8449,
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2019, 2, 13, 19, 48, 34, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1,
                "non_empty" => btreemap! {
                    "x" => "1",
                },
                "empty" => btreemap! {},
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        empty_sd_value {
            args: func_args![value: r#"<13>1 2019-02-13T19:48:34+00:00 74794bfb6795 root 8449 - [non_empty x=""][empty] qwerty"#],
            want: Ok(btreemap!{
                "message" => "qwerty",
                "appname" => "root",
                "facility" => "user",
                "hostname" => "74794bfb6795",
                "message" => "qwerty",
                "procid" => 8449,
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2019, 2, 13, 19, 48, 34, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1,
                "empty" => btreemap! {},
                "non_empty" => btreemap! {
                    "x" => "",
                },
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        non_structured_data_in_message {
            args: func_args![value: "<131>Jun 8 11:54:08 master apache_error [Tue Jun 08 11:54:08.929301 2021] [php7:emerg] [pid 1374899] [client 95.223.77.60:41888] rest of message"],
            want: Ok(btreemap!{
                "appname" => "apache_error",
                "facility" => "local0",
                "hostname" => "master",
                "severity" => "err",
                "timestamp" => jiff::civil::datetime(datetime::utc_now().year(), 6, 8, 11, 54, 8, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "message" => "[Tue Jun 08 11:54:08.929301 2021] [php7:emerg] [pid 1374899] [client 95.223.77.60:41888] rest of message",
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        escapes_in_structured_data_quote {
            args: func_args![value: r#"<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 key="hello \"test\""] An application event log entry..."#],
            want: Ok(btreemap!{
                "appname" => "evntslog",
                "exampleSDID@32473" => btreemap! {
                    "key" => r#"hello "test""#,
                },
                "facility" => "local4",
                "hostname" => "mymachine.example.com",
                "message" => "An application event log entry...",
                "msgid" => "ID47",
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2003, 10, 11, 22, 14, 15, 3_000_000).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        escapes_in_structured_data_slash {
            args: func_args![value: r#"<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 key="hello a\\b"] An application event log entry..."#],
            want: Ok(btreemap!{
                "appname" => "evntslog",
                "exampleSDID@32473" => btreemap! {
                    "key" => r"hello a\b",
                },
                "facility" => "local4",
                "hostname" => "mymachine.example.com",
                "message" => "An application event log entry...",
                "msgid" => "ID47",
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2003, 10, 11, 22, 14, 15, 3_000_000).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        escapes_in_structured_data_bracket {
            args: func_args![value: r#"<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 key="hello [bye\]"] An application event log entry..."#],
            want: Ok(btreemap!{
                "appname" => "evntslog",
                "exampleSDID@32473" => btreemap! {
                    "key" => "hello [bye]",
                },
                "facility" => "local4",
                "hostname" => "mymachine.example.com",
                "message" => "An application event log entry...",
                "msgid" => "ID47",
                "severity" => "notice",
                "timestamp" => jiff::civil::datetime(2003, 10, 11, 22, 14, 15, 3_000_000).to_zoned(jiff::tz::TimeZone::UTC).unwrap().timestamp(),
                "version" => 1
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
