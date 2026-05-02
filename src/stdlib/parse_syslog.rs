use crate::compiler::prelude::*;
use chrono::{DateTime, Datelike, Utc};
use std::collections::BTreeMap;
use syslog_loose::{IncompleteDate, Message, ProcId, Protocol, Variant};

pub(crate) fn parse_syslog(value: &Value, ctx: &Context) -> Resolved {
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

    fn usage(&self) -> &'static str {
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
    let now = Utc::now();
    if now.month() == 1 && month == 12 {
        now.year() - 1
    } else {
        now.year()
    }
}

/// Index of the closing `]` for an RFC 5424 structured-data element starting with `[`,
/// respecting quotes and escapes inside param values.
fn find_sd_element_closing_bracket(s: &str) -> Option<usize> {
    if !s.starts_with('[') {
        return None;
    }
    let mut in_string = false;
    let mut escape = false;
    for (i, c) in s.char_indices().skip(1) {
        if escape {
            escape = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
        } else {
            match c {
                '"' => in_string = true,
                ']' => return Some(i),
                _ => {}
            }
        }
    }
    None
}

/// Parse the inside of `[...]` as SD-ID and SD-PARAMs. Returns `None` if the slice is malformed.
fn parse_structured_data_element_inner(inner: &str) -> Option<(String, BTreeMap<String, String>)> {
    let inner = inner.trim();
    if inner.is_empty() {
        return None;
    }
    let id_end = inner
        .find(char::is_whitespace)
        .unwrap_or(inner.len());
    let id = inner.get(..id_end)?.trim();
    if id.is_empty() {
        return None;
    }
    let mut rest = inner.get(id_end..)?.trim_start();
    let mut params = BTreeMap::new();
    while !rest.is_empty() {
        let eq = rest.find('=')?;
        let name = rest.get(..eq)?.trim_end();
        if name.is_empty() {
            return None;
        }
        rest = rest.get(eq + 1..)?.trim_start();
        if !rest.starts_with('"') {
            return None;
        }
        rest = rest.get(1..)?;
        let mut value = String::new();
        let mut close_quote_idx = None;
        let mut esc = false;
        for (i, c) in rest.char_indices() {
            if esc {
                match c {
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    '"' | '\\' | ']' => value.push(c),
                    _ => {
                        value.push('\\');
                        value.push(c);
                    }
                }
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                close_quote_idx = Some(i);
                break;
            } else {
                value.push(c);
            }
        }
        let end = close_quote_idx?;
        params.insert(name.to_string(), value);
        rest = rest.get(end + 1..)?.trim_start();
    }
    Some((id.to_string(), params))
}

/// `syslog_loose` sometimes leaves further RFC 5424 SD elements in `msg` after the first block.
/// Split those leading `[sd-id ...]` segments from the human-readable message tail.
fn split_leading_rfc5424_structured_data(
    msg: &str,
) -> (Vec<(String, BTreeMap<String, String>)>, &str) {
    let mut parsed = Vec::new();
    let mut s = msg;
    loop {
        s = s.trim_start();
        if !s.starts_with('[') {
            break;
        }
        let Some(close) = find_sd_element_closing_bracket(s) else {
            break;
        };
        let inner = &s[1..close];
        let after = s.get(close + 1..).unwrap_or("");
        match parse_structured_data_element_inner(inner) {
            Some(item) => {
                parsed.push(item);
                s = after;
            }
            None => break,
        }
    }
    (parsed, s.trim_start())
}

/// Create a `Value::Map` from the fields of the given syslog message.
fn message_to_value(message: Message<&str>) -> Value {
    let mut result = BTreeMap::new();

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
            sdata.insert((*name).into(), value.into());
        }
        result.insert(element.id.to_string().into(), sdata.into());
    }

    let (extra_sd, cleaned_msg) = split_leading_rfc5424_structured_data(message.msg);
    for (id, params) in extra_sd {
        if result.contains_key(id.as_str()) {
            continue;
        }
        let mut sdata = BTreeMap::new();
        for (name, value) in params {
            sdata.insert(name.into(), value.into());
        }
        result.insert(id.into(), sdata.into());
    }

    result.insert(
        "message".to_string().into(),
        cleaned_msg.to_string().into(),
    );

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
    use chrono::{TimeZone, Timelike};

    use super::*;

    test_function![
        parse_syslog => ParseSyslog;

        valid {
            args: func_args![value: r#"<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut="3" eventSource= "Application" eventID="1011"] Try to override the THX port, maybe it will reboot the neural interface!"#],
            want: Ok(btreemap! {
                "severity" => "notice",
                "facility" => "user",
                "timestamp" => Utc.with_ymd_and_hms(2020, 3, 13, 20, 45, 38).unwrap().with_nanosecond(119_000_000).unwrap(),
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
                    "timestamp" => Utc.with_ymd_and_hms(Utc::now().year(), 6, 13, 16, 33, 35).unwrap(),
                    "appname" => "haproxy",
                    "procid" => 73411,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        missing_pri {
            args: func_args![value: "Jun 13 16:33:35 haproxy[73411]: I am missing a pri."],
            want: Ok(btreemap! {
                "message" => "I am missing a pri.",
                "timestamp" => Utc.with_ymd_and_hms(Utc::now().year(), 6, 13, 16, 33, 35).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2019, 2, 13, 19, 48, 34).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2019, 2, 13, 19, 48, 34).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2019, 2, 13, 19, 48, 34).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(Utc::now().year(), 6, 8, 11, 54, 8).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2003, 10, 11, 22, 14, 15).unwrap().with_nanosecond(3_000_000).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2003, 10, 11, 22, 14, 15).unwrap().with_nanosecond(3_000_000).unwrap(),
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
                "timestamp" => Utc.with_ymd_and_hms(2003, 10, 11, 22, 14, 15).unwrap().with_nanosecond(3_000_000).unwrap(),
                "version" => 1
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }

        rfc5424_multiple_structure_data_groups {
            args: func_args![value: r#"<134>1 2026-04-30T11:02:45.789-04:00 fw01 firewall 9876 TRAFFIC_ALLOW [conn@41058 src_ip="192.0.2.10" src_port="44321" protocol="tcp"] [dest@41058 dst_ip="198.51.100.25" dst_port="443" zone="internet"] [policy@41058 rule="allow_https" action="permit" log="true"] Allowed outbound HTTPS connection"#],
            want: Ok(btreemap! {
                "facility" => "local0",
                "severity" => "info",
                "timestamp" => Utc.with_ymd_and_hms(2026, 4, 30, 15, 2, 45).unwrap().with_nanosecond(789_000_000).unwrap(),
                "hostname" => "fw01",
                "appname" => "firewall",
                "procid" => 9876,
                "msgid" => "TRAFFIC_ALLOW",
                "conn@41058" => btreemap! {
                    "src_ip" => "192.0.2.10",
                    "src_port" => "44321",
                    "protocol" => "tcp",
                },
                "dest@41058" => btreemap! {
                    "dst_ip" => "198.51.100.25",
                    "dst_port" => "443",
                    "zone" => "internet",
                },
                "policy@41058" => btreemap! {
                    "rule" => "allow_https",
                    "action" => "permit",
                    "log" => "true",
                },
                "message" => "Allowed outbound HTTPS connection",
                "version" => 1,
            }),
            tdef: TypeDef::object(inner_kind()).fallible(),
        }
    ];
}
