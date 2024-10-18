use super::{
    grok_filter::apply_filter,
    parse_grok_rules::{GrokField, GrokRule},
};
use crate::path::parse_value_path;
use crate::value::{ObjectMap, Value};
use std::collections::BTreeMap;
use tracing::{error, warn};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("failed to apply filter '{}' to '{}'", .0, .1)]
    FailedToApplyFilter(String, String),
    #[error("value does not match any rule")]
    NoMatch,
    #[error("failure occurred during match of the pattern against the value: '{}'", .0)]
    FailedToMatch(String),
}

/// Parses a given source field value by applying the list of grok rules until the first match found.
pub fn parse_grok(source_field: &str, grok_rules: &[GrokRule]) -> Result<Value, Error> {
    for rule in grok_rules {
        match apply_grok_rule(source_field, rule) {
            Err(Error::NoMatch) => continue,
            other => return other,
        }
    }
    Err(Error::NoMatch)
}

/// Tries to parse a given string with a given grok rule.
/// Returns a result value or an error otherwise.
/// Errors:
/// - FailedToApplyFilter - matches the rule, but there was a runtime error while applying on of the filters
/// - NoMatch - this rule does not match a given string
/// - FailedToMatch - there was a runtime error while matching the compiled pattern against the source
fn apply_grok_rule(source: &str, grok_rule: &GrokRule) -> Result<Value, Error> {
    let mut parsed = Value::Object(BTreeMap::new());

    match grok_rule.pattern.match_against(source) {
        Ok(Some(matches)) => {
            for (name, match_str) in matches.iter() {
                if match_str.is_empty() {
                    continue;
                }

                let mut value = Some(Value::from(match_str));

                if let Some(GrokField {
                    lookup: field,
                    filters,
                }) = grok_rule.fields.get(name)
                {
                    filters.iter().for_each(|filter| {
                    if let Some(ref mut v) = value {
                        value = match apply_filter(v, filter) {
                            Ok(Value::Null) => None,
                            Ok(v) if v.is_object() => Some(parse_keys_as_path(v)),
                            Ok(v) => Some(v),
                            Err(error) => {
                                warn!(message = "Error applying filter", field = %field, filter = %filter, %error);
                                None
                            }
                        };
                    }
                });

                    if let Some(value) = value {
                        match value {
                            // root-level maps must be merged
                            Value::Object(map) if field.is_root() => {
                                parsed.as_object_mut().expect("root is object").extend(map);
                            }
                            // anything else at the root leve must be ignored
                            _ if field.is_root() => {}
                            // otherwise just apply VRL lookup insert logic
                            _ => match parsed.get(field).cloned() {
                                Some(Value::Array(mut values)) => {
                                    values.push(value);
                                    parsed.insert(field, values);
                                }
                                Some(v) => {
                                    parsed.insert(field, Value::Array(vec![v, value]));
                                }
                                None => {
                                    parsed.insert(field, value);
                                }
                            },
                        };
                    }
                } else {
                    // this must be a regex named capturing group (?<name>group),
                    // where name can only be alphanumeric - thus we do not need to parse field names(no nested fields)
                    parsed
                        .as_object_mut()
                        .expect("parsed value is not an object")
                        .insert(name.to_string().into(), value.into());
                }
            }

            postprocess_value(&mut parsed);
            Ok(parsed)
        }
        Ok(None) => Err(Error::NoMatch),
        Err(e) => Err(e),
    }
}

// parse all internal object keys as path
fn parse_keys_as_path(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut result = Value::Object(ObjectMap::new());
            for (k, v) in map.into_iter() {
                let path = parse_value_path(&k)
                    .unwrap_or_else(|_| crate::owned_value_path!(&k.to_string()));
                result.insert(&path, parse_keys_as_path(v));
            }
            result
        }
        Value::Array(a) => Value::Array(a.into_iter().map(parse_keys_as_path).collect()),
        v => v,
    }
}

/// postprocess parsed values
fn postprocess_value(value: &mut Value) {
    // remove empty objects
    match value {
        Value::Array(a) => a.iter_mut().for_each(postprocess_value),
        Value::Object(map) => {
            map.values_mut().for_each(postprocess_value);
            map.retain(|_, value| {
                !matches!(value, Value::Object(v) if v.is_empty()) && !matches!(value, Value::Null)
            })
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::btreemap;
    use crate::value::Value;
    use chrono::{Datelike, NaiveDate, Timelike, Utc};
    use ordered_float::NotNan;
    use tracing_test::traced_test;

    use super::super::parse_grok_rules::parse_grok_rules;
    use super::*;

    const FIXTURE_ROOT: &str = "tests/data/fixtures/parse_grok";

    #[test]
    fn parses_simple_grok() {
        let rules = parse_grok_rules(
            &[
                "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"
                    .to_string(),
            ],
            BTreeMap::new(),
        )
        .expect("couldn't parse rules");
        let parsed = parse_grok("2020-10-02T23:22:12.223222Z info Hello world", &rules).unwrap();

        assert_eq!(
            parsed,
            Value::from(btreemap! {
                "timestamp" => "2020-10-02T23:22:12.223222Z",
                "level" => "info",
                "message" => "Hello world"
            })
        );
    }

    #[test]
    fn parses_complex_grok() {
        let rules = parse_grok_rules(
            // patterns
            &[
                "%{access.common}".to_string(),
                r#"%{access.common} (%{number:duration:scale(1000000000)} )?"%{_referer}" "%{_user_agent}"( "%{_x_forwarded_for}")?.*"#.to_string()
            ],
            // aliases
            btreemap! {
                "access.common" => r#"%{_client_ip} %{_ident} %{_auth} \[%{_date_access}\] "(?>%{_method} |)%{_url}(?> %{_version}|)" %{_status_code} (?>%{_bytes_written}|-)"#.to_string(),
                "_auth" => r#"%{notSpace:http.auth:nullIf("-")}"#.to_string(),
                "_bytes_written" => "%{integer:network.bytes_written}".to_string(),
                "_client_ip" => "%{ipOrHost:network.client.ip}".to_string(),
                "_version" => r#"HTTP\/%{regex("\\d+\\.\\d+"):http.version}"#.to_string(),
                "_url" => "%{notSpace:http.url}".to_string(),
                "_ident" => "%{notSpace:http.ident}".to_string(),
                "_user_agent" => r#"%{regex("[^\\\"]*"):http.useragent}"#.to_string(),
                "_referer" => "%{notSpace:http.referer}".to_string(),
                "_status_code" => "%{integer:http.status_code}".to_string(),
                "_method" => "%{word:http.method}".to_string(),
                "_date_access" => "%{notSpace:date_access}".to_string(),
                "_x_forwarded_for" => r#"%{regex("[^\\\"]*"):http._x_forwarded_for:nullIf("-")}"#.to_string()}).expect("couldn't parse rules");
        let parsed = parse_grok(r#"127.0.0.1 - frank [13/Jul/2016:10:55:36] "GET /apache_pb.gif HTTP/1.0" 200 2326 0.202 "http://www.perdu.com/" "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36" "-""#, &rules).unwrap();

        assert_eq!(
            parsed,
            Value::from(btreemap! {
                "date_access" => "13/Jul/2016:10:55:36",
                "duration" => 202000000,
                "http" => btreemap! {
                    "auth" => "frank",
                    "ident" => "-",
                    "method" => "GET",
                    "status_code" => 200,
                    "url" => "/apache_pb.gif",
                    "version" => "1.0",
                    "referer" => "http://www.perdu.com/",
                    "useragent" => "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36",
                },
                "network" => btreemap! {
                    "bytes_written" => 2326,
                    "client" => btreemap! {
                        "ip" => "127.0.0.1"
                    }
                }
            })
        );
    }

    #[test]
    fn supports_matchers() {
        test_grok_pattern(vec![
            ("%{number:field}", "-1.2", Ok(Value::from(-1.2_f64))),
            ("%{number:field}", "-1", Ok(Value::from(-1))),
            ("%{numberExt:field}", "-1234e+3", Ok(Value::from(-1234000))),
            ("%{numberExt:field}", ".1e+3", Ok(Value::from(100))),
            ("%{integer:field}", "-2", Ok(Value::from(-2))),
            ("%{integerExt:field}", "+2", Ok(Value::from(2))),
            ("%{integerExt:field}", "-2", Ok(Value::from(-2))),
            ("%{integerExt:field}", "-1e+2", Ok(Value::from(-100))),
            ("%{integerExt:field}", "1234.1e+5", Err(Error::NoMatch)),
        ]);
    }

    #[test]
    fn supports_filters() {
        test_grok_pattern(vec![
            ("%{data:field:number}", "1.0", Ok(Value::from(1))),
            ("%{data:field:integer}", "1", Ok(Value::from(1))),
            (
                "%{data:field:lowercase}",
                "aBC",
                Ok(Value::Bytes("abc".into())),
            ),
            (
                "%{data:field:uppercase}",
                "Abc",
                Ok(Value::Bytes("ABC".into())),
            ),
            ("%{integer:field:scale(10)}", "1", Ok(Value::from(10))),
            ("%{number:field:scale(0.5)}", "10.0", Ok(Value::from(5))),
        ]);
    }

    fn test_grok_pattern(tests: Vec<(&str, &str, Result<Value, Error>)>) {
        for (filter, k, v) in tests {
            let rules =
                parse_grok_rules(&[filter.to_string()], BTreeMap::new()).unwrap_or_else(|error| {
                    panic!("failed to parse {k} with filter {filter}: {error}")
                });
            let parsed = parse_grok(k, &rules);

            if v.is_ok() {
                assert_eq!(
                    parsed.unwrap_or_else(|_| panic!("{filter} does not match {k}")),
                    Value::from(btreemap! {
                        "field" =>  v.unwrap(),
                    }),
                    "failed to parse {k} with filter {filter}"
                );
            } else {
                assert_eq!(parsed, v, "failed to parse {k} with filter {filter}");
            }
        }
    }

    fn test_full_grok(tests: Vec<(&str, &str, Result<Value, Error>)>) {
        for (filter, k, v) in tests {
            let rules = parse_grok_rules(&[filter.to_string()], BTreeMap::new())
                .unwrap_or_else(|_| panic!("failed to parse {k} with filter {filter}"));
            let parsed = parse_grok(k, &rules);

            assert_eq!(parsed, v, "failed to parse {k} with filter {filter}");
        }
    }

    #[test]
    fn fails_on_unknown_pattern_definition() {
        assert_eq!(
            parse_grok_rules(&["%{unknown}".to_string()], BTreeMap::new())
                .unwrap_err()
                .to_string(),
            r#"failed to parse grok expression '(?m)\A%{unknown}\z': The given pattern definition name "unknown" could not be found in the definition map"#
        );
    }

    #[test]
    fn fails_on_unknown_filter() {
        assert_eq!(
            parse_grok_rules(
                &["%{data:field:unknownFilter}".to_string()],
                BTreeMap::new(),
            )
            .unwrap_err()
            .to_string(),
            "unknown filter 'unknownFilter'"
        );
    }

    #[test]
    fn fails_on_invalid_matcher_parameter() {
        assert_eq!(
            parse_grok_rules(&["%{regex(1):field}".to_string()], BTreeMap::new())
                .unwrap_err()
                .to_string(),
            "invalid arguments for the function 'regex'"
        );
    }

    #[test]
    fn fails_on_invalid_filter_parameter() {
        assert_eq!(
            parse_grok_rules(&["%{data:field:scale()}".to_string()], BTreeMap::new())
                .unwrap_err()
                .to_string(),
            "invalid arguments for the function 'scale'"
        );
    }

    #[test]
    fn regex_with_empty_field() {
        test_grok_pattern(vec![(
            r#"%{regex("\\d+\\.\\d+")} %{data:field}"#,
            "1.0 field_value",
            Ok(Value::from("field_value")),
        )]);
    }

    #[test]
    fn does_not_merge_field_maps() {
        // only root-level maps are merged
        test_full_grok(vec![(
            "'%{data:nested.json:json}' '%{data:nested.json:json}'",
            r#"'{ "json_field1": "value2" }' '{ "json_field2": "value3" }'"#,
            Ok(Value::from(btreemap! {
                "nested" => btreemap! {
                    "json" =>  Value::Array(vec! [
                        Value::from(btreemap! { "json_field1" => Value::Bytes("value2".into()) }),
                        Value::from(btreemap! { "json_field2" => Value::Bytes("value3".into()) }),
                    ]),
                }
            })),
        )]);
    }

    #[test]
    fn supports_filters_without_fields() {
        // if the root-level value, after filters applied, is a map then merge it at the root level,
        // otherwise ignore it
        test_full_grok(vec![
            (
                "%{data::json}",
                r#"{ "json_field1": "value2" }"#,
                Ok(Value::from(btreemap! {
                    "json_field1" => Value::Bytes("value2".into()),
                })),
            ),
            (
                "%{notSpace:standalone_field} '%{data::json}' '%{data::json}' %{number::number}",
                r#"value1 '{ "json_field1": "value2" }' '{ "json_field2": "value3" }' 3"#,
                Ok(Value::from(btreemap! {
                    "standalone_field" => Value::Bytes("value1".into()),
                    "json_field1" => Value::Bytes("value2".into()),
                    "json_field2" => Value::Bytes("value3".into())
                })),
            ),
            // ignore non-map root-level fields
            (
                "%{notSpace:standalone_field} %{data::integer}",
                "value1 1",
                Ok(Value::from(btreemap! {
                    "standalone_field" => Value::Bytes("value1".into()),
                })),
            ),
            // empty map if fails
            (
                "%{data::json}",
                "not a json",
                Ok(Value::from(BTreeMap::new())),
            ),
        ]);
    }

    #[test]
    fn ignores_field_if_filter_fails() {
        // empty map for filters like json
        test_full_grok(vec![(
            "%{notSpace:field1:integer} %{data:field2:json}",
            "not_a_number not a json",
            Ok(Value::from(BTreeMap::new())),
        )]);
    }

    #[test]
    fn fails_on_no_match() {
        let rules = parse_grok_rules(
            &[
                "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}"
                    .to_string(),
            ],
            BTreeMap::new(),
        )
        .expect("couldn't parse rules");
        let error = parse_grok("an ungrokkable message", &rules).unwrap_err();

        assert_eq!(error, Error::NoMatch);
    }

    #[test]
    fn fails_on_too_many_match_retries() {
        let pattern = std::fs::read_to_string(format!(
            "{}/pattern/excessive-match-retries.txt",
            FIXTURE_ROOT
        ))
        .expect("Failed to read pattern file");
        let value = std::fs::read_to_string(format!(
            "{}/value/excessive-match-retries.txt",
            FIXTURE_ROOT
        ))
        .expect("Failed to read value file");

        let rules = parse_grok_rules(
            // patterns
            &[pattern],
            BTreeMap::new(),
        )
        .expect("couldn't parse rules");

        let parsed = parse_grok(&value, &rules);

        assert_eq!(
            parsed.unwrap_err(),
            Error::FailedToMatch("Regex search error in the underlying engine".to_string())
        )
    }

    #[test]
    fn appends_to_the_same_field() {
        let rules = parse_grok_rules(
            &[
                r#"%{integer:nested.field} %{notSpace:nested.field:uppercase} %{notSpace:nested.field:nullIf("-")}"#
                    .to_string(),
            ],
            BTreeMap::new(),
        )
            .expect("couldn't parse rules");
        let parsed = parse_grok("1 info message", &rules).unwrap();

        assert_eq!(
            parsed,
            Value::from(btreemap! {
                "nested" => btreemap! {
                   "field" =>  Value::Array(vec![1.into(), "INFO".into(), "message".into()]),
                },
            })
        );
    }

    #[test]
    fn error_on_circular_dependency() {
        let err = parse_grok_rules(
            // patterns
            &["%{pattern1}".to_string()],
            // aliases with a circular dependency
            btreemap! {
            "pattern1" => "%{pattern2}".to_string(),
            "pattern2" => "%{pattern1}".to_string()},
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Circular dependency found in the alias 'pattern1'"
        );
    }

    #[test]
    fn extracts_field_with_regex_capture() {
        test_grok_pattern(vec![(
            r"(?<field>\w+)",
            "abc",
            Ok(Value::Bytes("abc".into())),
        )]);

        // the group name can only be alphanumeric,
        // though we don't validate group names(it would be unnecessary overhead at boot-time),
        // field names are treated as literals, not as lookup paths
        test_full_grok(vec![(
            r"(?<nested.field.name>\w+)",
            "abc",
            Ok(Value::from(btreemap! {
                "nested.field.name" => Value::Bytes("abc".into()),
            })),
        )]);
    }

    #[test]
    fn supports_date_matcher() {
        let now = Utc::now();
        let now = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day())
            .unwrap()
            .and_hms_opt(12, 13, 14)
            .unwrap()
            .and_utc();
        test_grok_pattern(vec![
            (
                r#"%{date("dd/MMM/yyyy"):field}"#,
                "06/Mar/2013",
                Ok(Value::Integer(1362528000000)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466065743000)),
            ),
            (
                r#"%{date("dd/MMM/yyyy:HH:mm:ss Z"):field}"#,
                "06/Mar/2013:01:36:30 +0900",
                Ok(Value::Integer(1362501390000)),
            ),
            (
                r#"%{date("yyyy-MM-dd'T'HH:mm:ss.SSSZ"):field}"#,
                "2016-11-29T16:21:36.431+0000",
                Ok(Value::Integer(1480436496431)),
            ),
            (
                r#"%{date("yyyy-MM-dd'T'HH:mm:ss.SSSZZ"):field}"#,
                "2016-11-29T16:21:36.431+00:00",
                Ok(Value::Integer(1480436496431)),
            ),
            (
                r#"%{date("dd/MMM/yyyy:HH:mm:ss.SSS"):field}"#,
                "06/Feb/2009:12:14:14.655",
                Ok(Value::Integer(1233922454655)),
            ),
            (
                r#"%{date("yyyy-MM-dd HH:mm:ss.SSS z"):field}"#,
                "2007-08-31 19:22:22.427 CET",
                Ok(Value::Integer(1188580942427)),
            ),
            (
                r#"%{date("yyyy-MM-dd HH:mm:ss.SSS zzzz"):field}"#,
                "2007-08-31 19:22:22.427 America/Thule",
                Ok(Value::Integer(1188598942427)),
            ),
            (
                r#"%{date("yyyy-MM-dd HH:mm:ss.SSS Z"):field}"#,
                "2007-08-31 19:22:22.427 -03:00",
                Ok(Value::Integer(1188598942427)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy", "Europe/Moscow"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466054943000)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy", "UTC+5"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466047743000)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy", "+3"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466054943000)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy", "+03:00"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466054943000)),
            ),
            (
                r#"%{date("EEE MMM dd HH:mm:ss yyyy", "-0300"):field}"#,
                "Thu Jun 16 08:29:03 2016",
                Ok(Value::Integer(1466076543000)),
            ),
            (
                r#"%{date("MMM d y HH:mm:ss z"):field}"#,
                "Nov 16 2020 13:41:29 GMT",
                Ok(Value::Integer(1605534089000)),
            ),
            (
                r#"%{date("yyyy-MM-dd HH:mm:ss.SSSS"):field}"#,
                "2019-11-25 11:21:32.6282",
                Ok(Value::Integer(1574680892628)),
            ),
            (
                r#"%{date("yyyy-MM-dd'T'HH:mm:ss.SSSZ"):field}"#,
                "2016-09-02T15:02:29.648Z",
                Ok(Value::Integer(1472828549648)),
            ),
            (
                r#"%{date("yyMMdd HH:mm:ss"):field}"#,
                "171113 14:14:20",
                Ok(Value::Integer(1510582460000)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss z"):field}"#,
                "5/6/18 19:40:59 GMT",
                Ok(Value::Integer(1525635659000)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss z"):field}"#,
                "11/16/18 19:40:59 GMT",
                Ok(Value::Integer(1542397259000)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss,SSS z"):field}"#,
                "11/16/18 19:40:59,123 GMT",
                Ok(Value::Integer(1542397259123)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss,SSSS z"):field}"#,
                "11/16/18 19:40:59,1234 GMT",
                Ok(Value::Integer(1542397259123)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss,SSSSSSSSS z"):field}"#,
                "11/16/18 19:40:59,123456789 GMT",
                Ok(Value::Integer(1542397259123)),
            ),
            (
                r#"%{date("M/d/yy HH:mm:ss.SSSS z"):field}"#,
                "11/16/18 19:40:59.1234 GMT",
                Ok(Value::Integer(1542397259123)),
            ),
            // date is missing - assume the current day
            (
                r#"%{date("HH:mm:ss"):field}"#,
                &format!("{}:{}:{}", now.hour(), now.minute(), now.second()),
                Ok(Value::Integer(now.timestamp() * 1000)),
            ),
            // if the year is missing - assume the current year
            (
                r#"%{date("d/M HH:mm:ss"):field}"#,
                &format!(
                    "{}/{} {}:{}:{}",
                    now.day(),
                    now.month(),
                    now.hour(),
                    now.minute(),
                    now.second()
                ),
                Ok(Value::Integer(now.timestamp() * 1000)),
            ),
        ]);

        // check error handling
        assert_eq!(
            parse_grok_rules(
                &[r#"%{date("ABC:XYZ"):field}"#.to_string()],
                BTreeMap::new(),
            )
            .unwrap_err()
            .to_string(),
            "invalid arguments for the function 'date'"
        );
        assert_eq!(
            parse_grok_rules(
                &[r#"%{date("EEE MMM dd HH:mm:ss yyyy", "unknown timezone"):field}"#.to_string()],
                BTreeMap::new(),
            )
            .unwrap_err()
            .to_string(),
            "invalid arguments for the function 'date'"
        );
    }

    #[test]
    fn supports_array_filter() {
        test_grok_pattern(vec![
            (
                "%{data:field:array}",
                "[1,2]",
                Ok(Value::Array(vec!["1".into(), "2".into()])),
            ),
            (
                r#"%{data:field:array("\\t")}"#,
                "[1\t2]",
                Ok(Value::Array(vec!["1".into(), "2".into()])),
            ),
            (
                r#"%{data:field:array("[]","\\n")}"#,
                "[1\n2]",
                Ok(Value::Array(vec!["1".into(), "2".into()])),
            ),
            (
                r#"%{data:field:array("","-")}"#,
                "1-2",
                Ok(Value::Array(vec!["1".into(), "2".into()])),
            ),
            (
                "%{data:field:array(integer)}",
                "[1,2]",
                Ok(Value::Array(vec![1.into(), 2.into()])),
            ),
            (
                r#"%{data:field:array(";", integer)}"#,
                "[1;2]",
                Ok(Value::Array(vec![1.into(), 2.into()])),
            ),
            (
                r#"%{data:field:array("{}",";", integer)}"#,
                "{1;2}",
                Ok(Value::Array(vec![1.into(), 2.into()])),
            ),
            (
                "%{data:field:array(number)}",
                "[1,2]",
                Ok(Value::Array(vec![1.into(), 2.into()])),
            ),
            (
                "%{data:field:array(integer)}",
                "[1,2]",
                Ok(Value::Array(vec![1.into(), 2.into()])),
            ),
            (
                "%{data:field:array(scale(10))}",
                "[1,2.1]",
                Ok(Value::Array(vec![10.into(), 21.into()])),
            ),
            (
                r#"%{data:field:array(";", scale(10))}"#,
                "[1;2.1]",
                Ok(Value::Array(vec![10.into(), 21.into()])),
            ),
            (
                r#"%{data:field:array("{}",";", scale(10))}"#,
                "{1;2.1}",
                Ok(Value::Array(vec![10.into(), 21.into()])),
            ),
        ]);

        test_full_grok(vec![
            // not an array
            (
                "%{data:field:array}",
                "abc",
                Ok(Value::Object(BTreeMap::new())),
            ),
            // failed to apply value filter(values are strings)
            (
                "%{data:field:array(scale(10))}",
                "[a,b]",
                Ok(Value::Object(BTreeMap::new())),
            ),
        ]);
    }

    #[test]
    fn parses_keyvalue() {
        test_full_grok(vec![
            (
                "%{data::keyvalue}",
                "key=valueStr",
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            (
                "%{data::keyvalue}",
                "key=<valueStr>",
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            (
                "%{data::keyvalue}",
                r#""key"="valueStr""#,
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            (
                "%{data::keyvalue}",
                "'key'='valueStr'",
                Ok(Value::from(btreemap! {
                   "key" => "valueStr"
                })),
            ),
            (
                "%{data::keyvalue}",
                "<key>=<valueStr>",
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            (
                r#"%{data::keyvalue(":")}"#,
                "key:valueStr",
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            (
                r#"%{data::keyvalue(":", "/")}"#,
                r#"key:"/valueStr""#,
                Ok(Value::from(btreemap! {
                    "key" => "/valueStr"
                })),
            ),
            (
                r#"%{data::keyvalue(":", "/")}"#,
                "/key:/valueStr",
                Ok(Value::from(btreemap! {
                    "/key" => "/valueStr"
                })),
            ),
            (
                r#"%{data::keyvalue(":=", "", "{}")}"#,
                "key:={valueStr}",
                Ok(Value::from(btreemap! {
                    "key" => "valueStr"
                })),
            ),
            // ignore space after the delimiter(comma)
            (
                r#"%{data::keyvalue}"#,
                "key1=value1, key2=value2",
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                })),
            ),
            // allow space as a legit value character, but trim key/values
            (
                r#"%{data::keyvalue("="," ")}"#,
                "key1=value1, key2 = value 2 ",
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value 2",
                })),
            ),
            (
                r#"%{data::keyvalue("=", "", "", "|")}"#,
                "key1=value1|key2=value2",
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                })),
            ),
            (
                r#"%{data::keyvalue("=", "", "", "|")}"#,
                r#"key1="value1"|key2="value2""#,
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                })),
            ),
            (
                r#"%{data::keyvalue(":=","","<>")}"#,
                r#"key1:=valueStr key2:=</valueStr2> key3:="valueStr3""#,
                Ok(Value::from(btreemap! {
                    "key1" => "valueStr",
                    "key2" => "/valueStr2",
                })),
            ),
            (
                "%{data::keyvalue}",
                "key1=value1,key2=value2",
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                })),
            ),
            (
                "%{data::keyvalue}",
                "key1=value1;key2=value2",
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                })),
            ),
            (
                "%{data::keyvalue}",
                "key:=valueStr",
                Ok(Value::from(BTreeMap::new())),
            ),
            // empty key or null
            (
                "%{data::keyvalue}",
                "key1= key2=null key3=value3",
                Ok(Value::from(btreemap! {
                    "key3" => "value3"
                })),
            ),
            // empty value or null - comma-separated
            (
                "%{data::keyvalue}",
                "key1=,key2=null,key3= ,key4=value4",
                Ok(Value::from(btreemap! {
                    "key4" => "value4"
                })),
            ),
            // empty key
            (
                "%{data::keyvalue}",
                "=,=value",
                Ok(Value::from(BTreeMap::new())),
            ),
            // type inference
            (
                "%{data::keyvalue}",
                "float=1.2,boolean=true,null=null,string=abc,integer1=11,integer2=12",
                Ok(Value::from(btreemap! {
                    "float" => Value::Float(NotNan::new(1.2).expect("not a float")),
                    "boolean" => Value::Boolean(true),
                    "string" => Value::Bytes("abc".into()),
                    "integer1" => Value::Integer(11),
                    "integer2" => Value::Integer(12)
                })),
            ),
            // type inference with extra spaces around field delimiters
            (
                "%{data::keyvalue}",
                "float=1.2 , boolean=true , null=null    ,   string=abc , integer1=11  ,  integer2=12  ",
                Ok(Value::from(btreemap! {
                    "float" => Value::Float(NotNan::new(1.2).expect("not a float")),
                    "boolean" => Value::Boolean(true),
                    "string" => Value::Bytes("abc".into()),
                    "integer1" => Value::Integer(11),
                    "integer2" => Value::Integer(12)
                })),
            ),
            // spaces around key-value delimiter are not allowed
            (
                "%{data::keyvalue}",
                "key = valueStr",
                Ok(Value::from(BTreeMap::new())),
            ),
            (
                "%{data::keyvalue}",
                "key= valueStr",
                Ok(Value::from(BTreeMap::new())),
            ),
            (
                "%{data::keyvalue}",
                "key =valueStr",
                Ok(Value::from(BTreeMap::new())),
            ),
            (
                r#"%{data::keyvalue(":")}"#,
                "kafka_cluster_status:8ca7b736f0aa43e5",
                Ok(Value::from(btreemap! {
                    "kafka_cluster_status" => "8ca7b736f0aa43e5"
                })),
            ),
            (
                "%{data::keyvalue}",
                "field=2.0e",
                Ok(Value::from(btreemap! {
                "field" => "2.0e"
                })),
            ),
            (
                r#"%{data::keyvalue("=", "\\w.\\-_@:")}"#,
                "IN=eth0 OUT= MAC", // no value
                Ok(Value::from(btreemap! {
                    "IN" => "eth0"
                })),
            ),
            (
                "%{data::keyvalue}",
                "db.name=my_db,db.operation=insert",
                Ok(Value::from(btreemap! {
                    "db" => btreemap! {
                        "name" => "my_db",
                        "operation" => "insert",
                    }
                })),
            ),
            // capture all possilbe key-value pairs from the string
            (
                "%{data::keyvalue}",
                r#" , key1=value1 "key2"="value2",key3=value3 "#,
                Ok(Value::from(btreemap! {
                    "key1" => "value1",
                    "key2" => "value2",
                    "key3" => "value3",
                })),
            ),
            (
                r#"%{data::keyvalue(": ",",")}"#,
                r#"client: 217.92.148.44, server: localhost, request: "HEAD http://174.138.82.103:80/sql/sql-admin/ HTTP/1.1", host: "174.138.82.103""#,
                Ok(Value::from(btreemap! {
                    "client" => "217.92.148.44",
                    "host" => "174.138.82.103",
                    "request" => "HEAD http://174.138.82.103:80/sql/sql-admin/ HTTP/1.1",
                    "server" => "localhost",
                })),
            ),
            // append values with the same key
            (
                r#"%{data::keyvalue}"#,
                r#"a=1, a=1, a=2"#,
                Ok(Value::from(btreemap! {
                    "a" => vec![1, 1, 2]
                })),
            ),
            // trim string values
            (
                r#"%{data::keyvalue("="," ")}"#,
                r#"a= foo"#,
                Ok(Value::from(btreemap! {
                    "a" => "foo"
                })),
            ),
            // ignore if key contains spaces
            (
                r#"%{data::keyvalue("="," ")}"#,
                "a key=value",
                Ok(Value::from(btreemap! {})),
            ),
            // parses valid octal numbers (start with 0) as decimals
            (
                r#"%{data::keyvalue}"#,
                "a=07",
                Ok(Value::from(btreemap! {
                    "a" => 7
                })),
            ),
            // parses invalid octal numbers (start with 0) as strings
            (
                r#"%{data::keyvalue}"#,
                "a=08",
                Ok(Value::from(btreemap! {
                    "a" => "08"
                })),
            ),
        ]);
    }

    #[test]
    fn alias_and_main_rule_extract_same_fields_to_array() {
        let rules = parse_grok_rules(
            // patterns
            &["%{notSpace:field:number} %{alias}".to_string()],
            // aliases
            btreemap! {
                "alias" => "%{notSpace:field:integer}".to_string()
            },
        )
        .expect("couldn't parse rules");
        let parsed = parse_grok("1 2", &rules).unwrap();

        assert_eq!(
            parsed,
            Value::from(btreemap! {
                 "field" =>  Value::Array(vec![1.into(), 2.into()]),
            })
        );
    }

    #[test]
    fn alias_with_filter() {
        let rules = parse_grok_rules(
            // patterns
            &["%{alias:field:uppercase}".to_string()],
            // aliases
            btreemap! {
                "alias" => "%{notSpace:subfield1} %{notSpace:subfield2:integer}".to_string()
            },
        )
        .expect("couldn't parse rules");
        let parsed = parse_grok("a 1", &rules).unwrap();

        assert_eq!(
            parsed,
            Value::from(btreemap! {
                 "field" =>  Value::Bytes("A 1".into()),
                 "subfield1" =>  Value::Bytes("a".into()),
                 "subfield2" =>  Value::Integer(1)
            })
        );
    }

    #[test]
    #[traced_test]
    fn does_not_emit_error_log_on_alternatives_with_filters() {
        test_full_grok(vec![(
            "(%{integer:field_int}|%{data:field_str})",
            "abc",
            Ok(Value::from(btreemap! {
                "field_str" =>  Value::Bytes("abc".into()),
            })),
        )]);
        assert!(!logs_contain("Error applying filter"));
    }

    #[test]
    fn parses_grok_unsafe_field_names() {
        test_full_grok(vec![
            (
                r#"%{data:field["quoted name"]}"#,
                "abc",
                Ok(Value::from(btreemap! {
                "field" => btreemap! {
                    "quoted name" => "abc",
                    }
                })),
            ),
            (
                "%{data:@field-name-with-symbols$}",
                "abc",
                Ok(Value::from(btreemap! {
                "@field-name-with-symbols$" => "abc"})),
            ),
            (
                "%{data:@parent.$child}",
                "abc",
                Ok(Value::from(btreemap! {
                "@parent" => btreemap! {
                    "$child" => "abc",
                    }
                })),
            ),
        ]);
    }

    #[test]
    fn parses_with_new_lines() {
        test_full_grok(vec![
            // the DOTALL mode is enabled by default
            (
                "%{data:field}",
                "a\nb",
                Ok(Value::from(btreemap! {
                    "field" => "a\nb"
                })),
            ),
            // (?s) enables the DOTALL mode
            (
                "(?s)%{data:field}",
                "a\nb",
                Ok(Value::from(btreemap! {
                    "field" => "a\nb"
                })),
            ),
            (
                "%{data:line1}\n%{data:line2}",
                "a\nb",
                Ok(Value::from(btreemap! {
                    "line1" => "a",
                    "line2" => "b"
                })),
            ),
            // disable the DOTALL mode with (?-s)
            ("(?s)(?-s)%{data:field}", "a\nb", Err(Error::NoMatch)),
            // disable and then enable the DOTALL mode
            (
                "(?-s)%{data:field} (?s)%{data:field}",
                "abc d\ne",
                Ok(Value::from(btreemap! {
                    "field" => Value::Array(vec!["abc".into(), "d\ne".into()]),
                })),
            ),
        ]);
    }

    #[test]
    fn supports_rubyhash_filter() {
        test_grok_pattern(vec![(
            "%{data:field:rubyhash}",
            r#"{hello=>"world",'number'=>42.0}"#,
            Ok(Value::from(btreemap! {
                "hello" => "world",
                "number" =>  42.0
            })),
        )]);
    }

    #[test]
    fn supports_querystring_filter() {
        test_grok_pattern(vec![(
            "%{data:field:querystring}",
            "foo=bar",
            Ok(Value::from(btreemap! {
                "foo" => "bar",
            })),
        )]);
    }

    #[test]
    fn supports_boolean_filter() {
        test_grok_pattern(vec![
            ("%{data:field:boolean}", "True", Ok(Value::Boolean(true))),
            (
                "%{data:field:boolean}",
                "NotTrue",
                Ok(Value::Boolean(false)),
            ),
        ]);
    }

    #[test]
    fn supports_decodeuricomponent_filter() {
        test_grok_pattern(vec![(
            "%{data:field:decodeuricomponent}",
            "%2Fservice%2Ftest",
            Ok(Value::Bytes("/service/test".into())),
        )]);
    }

    #[test]
    fn supports_xml_filter() {
        test_grok_pattern(vec![(
            "%{data:field:xml}",
            r#"<book category="CHILDREN">
                  <title lang="en">Harry Potter</title>
                  <author>J K. Rowling</author>
                  <year>2005</year>
                  <booleanValue>true</booleanValue>
                  <nullValue>null</nullValue>
                </book>"#,
            Ok(Value::from(btreemap! {
            "book" => btreemap! {
              "year" => "2005",
              "category" => "CHILDREN",
              "author" => "J K. Rowling",
              "booleanValue" => "true",
              "nullValue" => "null",
              "title" => btreemap! {
                "lang" => "en",
                "value" => "Harry Potter"
              }
            }
            })),
        )]);
    }

    #[test]
    fn parses_sample() {
        test_full_grok(vec![(
            r#"\[%{date("yyyy-MM-dd HH:mm:ss,SSS"):date}\]\[%{notSpace:level}\s*\]\[%{notSpace:logger.thread_name}-#%{integer:logger.thread_id}\]\[%{notSpace:logger.name}\] .*"#,
            r#"[2020-04-03 07:01:55,248][INFO ][exchange-worker-#43][FileWriteAheadLogManager] Started write-ahead log manager [mode=LOG_ONLY]"#,
            Ok(Value::from(btreemap! {
              "date"=> 1585897315248_i64,
              "level"=> "INFO",
              "logger"=> btreemap! {
                "name"=> "FileWriteAheadLogManager",
                "thread_id"=> 43,
                "thread_name"=> "exchange-worker"
              }
            })),
        )]);
    }

    #[test]
    fn remove_empty_objects() {
        test_full_grok(vec![
            (
                "%{data::json}",
                r#"{"root": {"object": {"empty": {}}, "string": "abc" }}"#,
                Ok(Value::Object(btreemap!(
                    "root" => btreemap! (
                        "string" => "abc"
                    )
                ))),
            ),
            (
                "%{data:field:json}",
                r#"{"root": {"object": {"empty": {}}, "string": "abc" }}"#,
                Ok(Value::Object(btreemap!(
                    "field" => btreemap!(
                        "root" => btreemap! (
                            "string" => "abc"
                        )
                )))),
            ),
            (
                r#"%{notSpace:network.destination.ip:nullIf("-")}"#,
                "-",
                Ok(Value::Object(btreemap!())),
            ),
        ]);
    }
    #[test]
    fn parses_json_keys_as_path() {
        test_full_grok(vec![(
            "%{data::json}",
            r#"{"a.b": "c"}"#,
            Ok(Value::Object(btreemap!(
                "a" => btreemap! (
                    "b" => "c"
                )
            ))),
        )]);
    }
}
