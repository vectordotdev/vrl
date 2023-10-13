use crate::compiler::prelude::*;
use chrono::{DateTime, Datelike, Utc};
use std::collections::BTreeMap;
use syslog_loose::{IncompleteDate, Message, ProcId, Protocol, Variant};

pub(crate) fn parse_syslog(value: Value, ctx: &Context) -> Resolved {
    let message = value.try_bytes_utf8_lossy()?;
    let timezone = match ctx.timezone() {
        TimeZone::Local => None,
        TimeZone::Named(tz) => Some(*tz),
    };
    let parsed = syslog_loose::parse_message_with_year_exact_tz(
        &message,
        resolve_year,
        timezone,
        Variant::Either,
    )?;
    Ok(message_to_value(parsed))
}

#[derive(Clone, Copy, Debug)]
pub struct ParseSyslog;

impl Function for ParseSyslog {
    fn identifier(&self) -> &'static str {
        "parse_syslog"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "parse syslog",
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

        parse_syslog(value, ctx)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::object(inner_kind()).fallible()
    }
}

/// Function used to resolve the year for syslog messages that don't include the
/// year. If the current month is January, and the syslog message is for
/// December, it will take the previous year. Otherwise, take the current year.
fn resolve_year((month, _date, _hour, _min, _sec): IncompleteDate) -> i32 {
    let now = Utc::now();
    if now.month() == 1 && month == 12 {
        now.year() - 1
    } else {
        now.year()
    }
}

/// Create a `Value::Map` from the fields of the given syslog message.
fn message_to_value(message: Message<&str>) -> Value {
    let mut result = BTreeMap::new();

    result.insert("message".to_string().into(), message.msg.to_string().into());

    if let Some(host) = message.hostname {
        result.insert("hostname".to_string().into(), host.to_string().into());
    }

    if let Some(severity) = message.severity {
        result.insert(
            "severity".to_string().into(),
            severity.as_str().to_owned().into(),
        );
    }

    if let Some(facility) = message.facility {
        result.insert(
            "facility".to_string().into(),
            facility.as_str().to_owned().into(),
        );
    }

    if let Protocol::RFC5424(version) = message.protocol {
        result.insert("version".to_string().into(), version.into());
    }

    if let Some(app_name) = message.appname {
        result.insert("appname".to_string().into(), app_name.to_owned().into());
    }

    if let Some(msg_id) = message.msgid {
        result.insert("msgid".to_string().into(), msg_id.to_owned().into());
    }

    if let Some(timestamp) = message.timestamp {
        let timestamp: DateTime<Utc> = timestamp.into();
        result.insert("timestamp".to_string().into(), timestamp.into());
    }

    if let Some(procid) = message.procid {
        let value: Value = match procid {
            ProcId::PID(pid) => pid.into(),
            ProcId::Name(name) => name.to_string().into(),
        };
        result.insert("procid".to_string().into(), value);
    }

    for element in message.structured_data {
        let mut sdata = BTreeMap::new();
        for (name, value) in element.params() {
            sdata.insert(name.to_string().into(), value.into());
        }
        result.insert(element.id.to_string().into(), sdata.into());
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
    use chrono::TimeZone;

    use super::*;

    test_function![
        parse_syslog => ParseSyslog;

        valid {
            args: func_args![value: r#"<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut="3" eventSource= "Application" eventID="1011"] Try to override the THX port, maybe it will reboot the neural interface!"#],
            want: Ok(btreemap! {
                "severity" => "notice",
                "facility" => "user",
                "timestamp" => chrono::Utc.ymd(2020, 3, 13).and_hms_milli(20, 45, 38, 119),
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
            args: func_args![value: r#"<133>Jun 13 16:33:35 haproxy[73411]: Proxy sticky-servers started."#],
            want: Ok(btreemap! {
                    "facility" => "local0",
                    "severity" => "notice",
                    "message" => "Proxy sticky-servers started.",
                    "timestamp" => chrono::Utc.ymd(Utc::now().year(), 6, 13).and_hms_milli(16, 33, 35, 0),
                    "appname" => "haproxy",
                    "procid" => 73411,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        missing_pri {
            args: func_args![value: r#"Jun 13 16:33:35 haproxy[73411]: I am missing a pri."#],
            want: Ok(btreemap! {
                "message" => "I am missing a pri.",
                "timestamp" => chrono::Utc.ymd(Utc::now().year(), 6, 13).and_hms_milli(16, 33, 35, 0),
                "appname" => "haproxy",
                "procid" => 73411,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        empty_sd_element {
            args: func_args![value: r#"<13>1 2019-02-13T19:48:34+00:00 74794bfb6795 root 8449 - [empty] qwerty"#],
            want: Ok(btreemap!{
                "message" => "qwerty",
                "appname" => "root",
                "facility" => "user",
                "hostname" => "74794bfb6795",
                "message" => "qwerty",
                "procid" => 8449,
                "severity" => "notice",
                "timestamp" => chrono::Utc.ymd(2019, 2, 13).and_hms_milli(19, 48, 34, 0),
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
                "timestamp" => chrono::Utc.ymd(2019, 2, 13).and_hms_milli(19, 48, 34, 0),
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
                "timestamp" => chrono::Utc.ymd(2019, 2, 13).and_hms_milli(19, 48, 34, 0),
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
                "timestamp" => chrono::Utc.ymd(chrono::Utc::now().year(), 6, 8).and_hms_milli(11, 54, 8, 0),
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
                "timestamp" => chrono::Utc.ymd(2003, 10, 11).and_hms_milli(22,14,15,3),
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
                "timestamp" => chrono::Utc.ymd(2003, 10, 11).and_hms_milli(22,14,15,3),
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
                "timestamp" => chrono::Utc.ymd(2003,10,11).and_hms_milli(22,14,15,3),
                "version" => 1
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
