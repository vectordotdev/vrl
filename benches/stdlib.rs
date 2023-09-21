#![allow(deprecated)]

use chrono::{DateTime, Datelike, TimeZone, Utc};
use criterion::{criterion_group, criterion_main, Criterion};
use regex::Regex;

use vrl::compiler::prelude::*;
use vrl::{bench_function, btreemap, func_args, value};

use crate::value::Value;

criterion_group!(
    name = benches;
    // encapsulates CI noise we saw in
    // https://github.com/vectordotdev/vector/pull/6408
    config = Criterion::default().noise_threshold(0.05);
    targets = array,
              assert,
              assert_eq,
              r#bool,
              ceil,
              chunks,
              compact,
              contains,
              decode_base16,
              decode_base64,
              decode_percent,
              decrypt,
              // TODO: Cannot pass a Path to bench_function
              //del,
              downcase,
              encode_base16,
              encode_base64,
              encode_key_value,
              encode_json,
              encode_logfmt,
              encode_percent,
              encrypt,
              ends_with,
              // TODO: Cannot pass a Path to bench_function
              //exists
              find,
              flatten,
              floor,
              float,
              format_int,
              format_number,
              format_timestamp,
              get,
              get_env_var,
              get_hostname,
              includes,
              int,
              ip_aton,
              ip_cidr_contains,
              ip_ntoa,
              ip_ntop,
              ip_pton,
              ip_subnet,
              ip_to_ipv6,
              ipv6_to_ipv4,
              is_array,
              is_boolean,
              is_empty,
              is_float,
              is_integer,
              is_ipv4,
              is_ipv6,
              is_json,
              is_null,
              is_nullish,
              is_object,
              is_regex,
              is_string,
              is_timestamp,
              join,
              keys,
              length,
              log,
              r#match,
              match_any,
              match_array,
              match_datadog_query,
              md5,
              merge,
              r#mod,
              // TODO: value is dynamic so we cannot assert equality
              //now,
              object,
              parse_apache_log,
              parse_aws_alb_log,
              parse_aws_cloudwatch_log_subscription_message,
              parse_aws_vpc_flow_log,
              parse_common_log,
              parse_csv,
              parse_duration,
              parse_glog,
              parse_grok,
              parse_groks,
              parse_key_value,
              parse_klog,
              parse_int,
              parse_json,
              parse_nginx_log,
              parse_query_string,
              parse_regex,
              parse_regex_all,
              parse_ruby_hash,
              parse_syslog,
              parse_timestamp,
              parse_tokens,
              parse_url,
              parse_user_agent,
              parse_xml,
              push,
              // TODO: value is non-deterministic and so cannot assert equality
              // random_bool,
              // TODO: value is non-deterministic and so cannot assert equality
              //random_float,
              random_int,
              redact,
              remove,
              replace,
              reverse_dns,
              round,
              seahash,
              set,
              sha1,
              sha2,
              sha3,
              slice,
              split,
              starts_with,
              string,
              strip_ansi_escape_codes,
              strip_whitespace,
              strlen,
              tally,
              tally_value,
              timestamp,
              to_bool,
              to_float,
              to_int,
              to_regex,
              to_string,
              to_syslog_facility,
              to_syslog_level,
              to_syslog_severity,
              to_unix_timestamp,
              truncate,
              unique,
              // TODO: Cannot pass a Path to bench_function
              //unnest
              // TODO: value is dynamic so we cannot assert equality
              //uuidv4,
              upcase,
              values,
              community_id,
);
criterion_main!(benches);

bench_function! {
    encrypt => vrl::stdlib::Encrypt;

    test {
        args: func_args![algorithm: value!("AES-128-OFB"), plaintext: value!("plaintext"), key: value!("1234567890123456"), iv: value!("1234567890123456") ],
        want: Ok(value!(b"\x05\x10\xace\xb2(\xf5\x92\xaf")),
    }
}

bench_function! {
    decrypt => vrl::stdlib::Decrypt;

    test {
        args: func_args![algorithm: value!("AES-128-OFB"), ciphertext: value!(b"\x05\x10\xace\xb2(\xf5\x92\xaf"), key: value!("1234567890123456"), iv: value!("1234567890123456") ],
        want: Ok(value!("plaintext")),
    }
}

bench_function! {
    append => vrl::stdlib::Append;

    arrays {
        args: func_args![value: value!([1, 2, 3]), items: value!([4, 5, 6])],
        want: Ok(value!([1, 2, 3, 4, 5, 6])),
    }
}

bench_function! {
    array => vrl::stdlib::Array;

    array {
        args: func_args![value: value!([1,2,3])],
        want: Ok(value!([1,2,3])),
    }
}

bench_function! {
    assert => vrl::stdlib::Assert;

    literal {
        args: func_args![condition: value!(true), message: "must be true"],
        want: Ok(value!(true)),
    }
}

bench_function! {
    assert_eq=> vrl::stdlib::AssertEq;

    literal {
        args: func_args![left: value!(true), right: value!(true), message: "must be true"],
        want: Ok(value!(true)),
    }
}

bench_function! {
    r#bool => vrl::stdlib::Boolean;

    r#bool {
        args: func_args![value: value!(true)],
        want: Ok(value!(true)),
    }
}

bench_function! {
    ceil => vrl::stdlib::Ceil;

    literal {
        args: func_args![value: 1234.56725, precision: 4],
        want: Ok(1234.5673),
    }
}

bench_function! {
    chunks => vrl::stdlib::Chunks;

    literal {
        args: func_args![value: "abcdefgh", chunk_size: 4],
        want: Ok(value!(["abcd", "efgh"])),
    }
}

bench_function! {
    compact => vrl::stdlib::Compact;

    array {
        args: func_args![
            value: value!([null, 1, "" ]),
        ],
        want: Ok(value!([ 1 ])),
    }

    map {
        args: func_args![
            value: value!({ "key1": null, "key2":  1, "key3": "" }),
        ],
        want: Ok(value!({ "key2": 1 })),
    }
}

bench_function! {
    contains => vrl::stdlib::Contains;

    case_sensitive {
        args: func_args![value: "abcdefg", substring: "cde", case_sensitive: true],
        want: Ok(value!(true)),
    }

    case_insensitive {
        args: func_args![value: "abcdefg", substring: "CDE", case_sensitive: false],
        want: Ok(value!(true)),
    }
}

bench_function! {
    decode_base16 => vrl::stdlib::DecodeBase16;

    literal {
        args: func_args![value: "736f6d652b3d737472696e672f76616c7565"],
        want: Ok("some+=string/value"),
    }
}

bench_function! {
    decode_base64 => vrl::stdlib::DecodeBase64;

    literal {
        args: func_args![value: "c29tZSs9c3RyaW5nL3ZhbHVl"],
        want: Ok("some+=string/value"),
    }
}

bench_function! {
    decode_percent => vrl::stdlib::DecodePercent;

    literal {
        args: func_args![value: "foo%20bar%3F"],
        want: Ok("foo bar?"),
    }
}

bench_function! {
    decode_mime_q => vrl::stdlib::DecodeMimeQ;

    base_64 {
        args: func_args![value: "=?utf-8?b?SGVsbG8sIFdvcmxkIQ==?="],
        want: Ok("Hello, World!"),
    }

    quoted_printable {
        args: func_args![value: "Subject: =?iso-8859-1?Q?=A1Hola,_se=F1or!?="],
        want: Ok("Subject: ¡Hola, señor!"),
    }
}

bench_function! {
    downcase => vrl::stdlib::Downcase;

    literal {
        args: func_args![value: "FOO"],
        want: Ok("foo")
    }
}

bench_function! {
    encode_base16 => vrl::stdlib::EncodeBase16;

    literal {
        args: func_args![value: "some+=string/value"],
        want: Ok("736f6d652b3d737472696e672f76616c7565"),
    }
}

bench_function! {
    encode_base64 => vrl::stdlib::EncodeBase64;

    literal {
        args: func_args![value: "some+=string/value"],
        want: Ok("c29tZSs9c3RyaW5nL3ZhbHVl"),
    }
}

bench_function! {
    encode_key_value => vrl::stdlib::EncodeKeyValue;

    encode_complex_value {
        args: func_args![value:
            btreemap! {
                "msg" => r#"result: {"authz": false, "length": 42}\n"#,
                "severity" => "    panic"
            },
            key_value_delimiter: "==",
            field_delimiter: "!!!"
            ],
        want: Ok(r#"msg=="result: {\"authz\": false, \"length\": 42}\\n"!!!severity=="    panic""#),
    }

    encode_key_value {
        args: func_args![value:
            btreemap! {
                "mow" => "vvo",
                "vvo" => "pkc",
                "pkc" => "hrb",
                "hrb" => "tsn",
                "tsn" => "can",
                "can" => "pnh",
                "pnh" => "sin",
                "sin" => "syd"
            },
            key_value_delimiter: ":",
            field_delimiter: ","
        ],
        want: Ok(r#"can:pnh,hrb:tsn,mow:vvo,pkc:hrb,pnh:sin,sin:syd,tsn:can,vvo:pkc"#),
    }

    fields_ordering {
        args: func_args![value:
            btreemap! {
                "mow" => "vvo",
                "vvo" => "pkc",
                "pkc" => "hrb",
                "hrb" => "tsn",
                "tsn" => "can",
                "can" => "pnh",
                "pnh" => "sin",
                "sin" => "syd"
            },
            fields_ordering: value!(["mow", "vvo", "pkc", "hrb", "tsn", "can", "pnh", "sin"]),
            key_value_delimiter: ":",
            field_delimiter: ","
        ],
        want: Ok(r#"mow:vvo,vvo:pkc,pkc:hrb,hrb:tsn,tsn:can,can:pnh,pnh:sin,sin:syd"#),
    }
}

bench_function! {
    encode_json => vrl::stdlib::EncodeJson;

    map {
        args: func_args![value: value![{"field": "value"}]],
        want: Ok(r#"{"field":"value"}"#),
    }
}

bench_function! {
    encode_logfmt => vrl::stdlib::EncodeLogfmt;

    string_with_characters_to_escape {
        args: func_args![value:
            btreemap! {
                "lvl" => "info",
                "msg" => r#"payload: {"code": 200}\n"#
            }],
        want: Ok(r#"lvl=info msg="payload: {\"code\": 200}\\n""#),
    }

    fields_ordering {
        args: func_args![value:
            btreemap! {
                "lvl" => "info",
                "msg" => "This is a log message",
                "log_id" => 12345,
            },
            fields_ordering: value!(["lvl", "msg"])
        ],
        want: Ok(r#"lvl=info msg="This is a log message" log_id=12345"#),
    }
}

bench_function! {
    encode_percent => vrl::stdlib::EncodePercent;

    non_alphanumeric {
        args: func_args![value: r#"foo bar?"#],
        want: Ok(r#"foo%20bar%3F"#),
    }

    controls {
        args: func_args![value: r#"foo bar"#, ascii_set: "CONTROLS"],
        want: Ok(r#"foo %14bar"#),
    }
}

bench_function! {
    ends_with => vrl::stdlib::EndsWith;

    case_sensitive {
        args: func_args![value: "abcdefg", substring: "efg", case_sensitive: true],
        want: Ok(value!(true)),
    }

    case_insensitive {
        args: func_args![value: "abcdefg", substring: "EFG", case_sensitive: false],
        want: Ok(value!(true)),
    }
}

bench_function! {
    find => vrl::stdlib::Find;

    str_matching {
        args: func_args![value: "foobarfoo", pattern: "bar"],
        want: Ok(value!(3)),
    }

    str_too_long {
        args: func_args![value: "foo", pattern: "foobar"],
        want: Ok(value!(-1)),
    }

    regex_matching_start {
        args: func_args![value: "foobar", pattern: Value::Regex(Regex::new("fo+z?").unwrap().into())],
        want: Ok(value!(0)),
    }
}

bench_function! {
    flatten => vrl::stdlib::Flatten;

    nested_map {
        args: func_args![value: value!({parent: {child1: 1, child2: 2}, key: "val"})],
        want: Ok(value!({"parent.child1": 1, "parent.child2": 2, key: "val"})),
    }

    nested_array {
        args: func_args![value: value!([42, [43, 44]])],
        want: Ok(value!([42, 43, 44])),
    }

    map_and_array {
        args: func_args![value: value!({
            "parent": {
                "child1": [1, [2, 3]],
                "child2": {"grandchild1": 1, "grandchild2": [1, [2, 3], 4]},
            },
            "key": "val",
        })],
        want: Ok(value!({
            "parent.child1": [1, [2, 3]],
            "parent.child2.grandchild1": 1,
            "parent.child2.grandchild2": [1, [2, 3], 4],
            "key": "val",
        })),
    }
}

bench_function! {
    float => vrl::stdlib::Float;

    float {
        args: func_args![value: value!(1.2)],
        want: Ok(value!(1.2)),
    }
}

bench_function! {
    floor  => vrl::stdlib::Floor;

    literal {
        args: func_args![value: 1234.56725, precision: 4],
        want: Ok(1234.5672),
    }
}

bench_function! {
    format_int => vrl::stdlib::FormatInt;

    decimal {
        args: func_args![value: 42],
        want: Ok("42"),
    }

    hexadecimal {
        args: func_args![value: 42, base: 16],
        want: Ok(value!("2a")),
    }
}

bench_function! {
    format_number => vrl::stdlib::FormatNumber;

    literal {
        args: func_args![
            value: 11222333444.56789,
            scale: 3,
            decimal_separator: ",",
            grouping_separator: "."
        ],
        want: Ok("11.222.333.444,567"),
    }
}

bench_function! {
    format_timestamp => vrl::stdlib::FormatTimestamp;

    iso_6801 {
        args: func_args![value: Utc.timestamp_opt(10, 0).single().expect("invalid timestamp"), format: "%+"],
        want: Ok("1970-01-01T00:00:10+00:00"),
    }
}

bench_function! {
    get_env_var => vrl::stdlib::GetEnvVar;

    // CARGO is set by `cargo`
    get {
        args: func_args![name: "CARGO"],
        want: Ok(env!("CARGO")),
    }
}

bench_function! {
    get_hostname => vrl::stdlib::GetHostname;

    get {
        args: func_args![],
        want: Ok(hostname::get().unwrap().to_string_lossy()),
    }
}

bench_function! {
    includes => vrl::stdlib::Includes;

    mixed_included_string {
        args: func_args![value: value!(["foo", 1, true, [1,2,3]]), item: value!("foo")],
        want: Ok(value!(true)),
    }
}

bench_function! {
    seahash => vrl::stdlib::Seahash;

    literal {
        args: func_args![value: "foo"],
        want: Ok(4_413_582_353_838_009_230_i64),
    }
}

bench_function! {
    set => vrl::stdlib::Set;

    single {
        args: func_args![value: value!({ "foo": "bar" }), path: vec!["baz"], data: true],
        want: Ok(value!({ "foo": "bar", "baz": true })),
    }

    nested {
        args: func_args![value: value!({ "foo": { "bar": "baz" } }), path: vec!["foo", "bar", "qux"], data: 42],
        want: Ok(value!({ "foo": { "bar": { "qux": 42 } } })),
    }

    indexing {
        args: func_args![value: value!([0, 42, 91]), path: vec![3], data: 1],
        want: Ok(value!([0, 42, 91, 1])),
    }
}

bench_function! {
    int => vrl::stdlib::Integer;

    int {
        args: func_args![value: value!(1)],
        want: Ok(value!(1)),
    }
}

bench_function! {
    ip_aton => vrl::stdlib::IpAton;

    valid {
        args: func_args![value: "1.2.3.4"],
        want: Ok(value!(16909060)),
    }
}

bench_function! {
    ip_cidr_contains => vrl::stdlib::IpCidrContains;

    ipv4 {
        args: func_args![cidr: "192.168.0.0/16", value: "192.168.10.32"],
        want: Ok(true),
    }

    ipv6 {
        args: func_args![cidr: "2001:4f8:3:ba::/64", value: "2001:4f8:3:ba:2e0:81ff:fe22:d1f1"],
        want: Ok(true),
    }
}

bench_function! {
    ip_ntoa => vrl::stdlib::IpNtoa;

    valid {
        args: func_args![value: 16909060],
        want: Ok(value!("1.2.3.4")),
    }
}

bench_function! {
    ip_ntop => vrl::stdlib::IpNtop;

    ipv4 {
        args: func_args![value: "1.2.3.4"],
        want: Ok(value!("\x01\x02\x03\x04")),
    }

    ipv6 {
        args: func_args![value: "102:304:506:708:90a:b0c:d0e:f10"],
        want: Ok(value!("\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10")),
    }
}

bench_function! {
    ip_pton => vrl::stdlib::IpPton;

    ipv4 {
        args: func_args![value: "\x01\x02\x03\x04"],
        want: Ok(value!("1.2.3.4")),
    }

    ipv6 {
        args: func_args![value: "\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"],
        want: Ok(value!("102:304:506:708:90a:b0c:d0e:f10")),
    }
}

bench_function! {
    ip_subnet => vrl::stdlib::IpSubnet;

    ipv4_mask {
        args: func_args![value: "192.168.10.23", subnet: "255.255.0.0"],
        want: Ok("192.168.0.0"),
    }

    ipv4_prefix {
        args: func_args![value: "192.168.10.23", subnet: "/16"],
        want: Ok("192.168.0.0"),
    }

    ipv6_mask {
        args: func_args![value: "2400:6800:4003:c02::64", subnet: "ff00::"],
        want: Ok("2400::"),
    }

    ipv6_prefix {
        args: func_args![value: "2400:6800:4003:c02::64", subnet: "/16"],
        want: Ok("2400::"),
    }
}

bench_function! {
    ip_to_ipv6 => vrl::stdlib::IpToIpv6;

    ipv4 {
        args: func_args![value: "192.168.0.1"],
        want: Ok("::ffff:192.168.0.1"),
    }

    ipv6 {
        args: func_args![value: "2404:6800:4003:c02::64"],
        want: Ok("2404:6800:4003:c02::64"),
    }
}

bench_function! {
    ipv6_to_ipv4 => vrl::stdlib::Ipv6ToIpV4;

    ipv6 {
        args: func_args![value: "::ffff:192.168.0.1"],
        want: Ok("192.168.0.1"),
    }

    ipv4 {
        args: func_args![value: "198.51.100.16"],
        want: Ok("198.51.100.16"),
    }
}

bench_function! {
    is_array => vrl::stdlib::IsArray;

    string {
        args: func_args![value: "foobar"],
        want: Ok(false),
    }

    array {
        args: func_args![value: value!([1, 2, 3])],
        want: Ok(true),
    }
}

bench_function! {
    is_boolean => vrl::stdlib::IsBoolean;

    string {
        args: func_args![value: "foobar"],
        want: Ok(false),
    }

    boolean {
        args: func_args![value: true],
        want: Ok(true),
    }
}

bench_function! {
    is_empty => vrl::stdlib::IsEmpty;

    empty_array {
        args: func_args![value: value!([])],
        want: Ok(true),
    }

    non_empty_array {
        args: func_args![value: value!([1, 2, 3])],
        want: Ok(false),
    }

    empty_object {
        args: func_args![value: value!({})],
        want: Ok(true),
    }

    non_empty_object {
        args: func_args![value: value!({"foo": "bar"})],
        want: Ok(false),
    }

    string {
        args: func_args![value: "foo"],
        want: Ok(false),
    }
}

bench_function! {
    is_float => vrl::stdlib::IsFloat;

    array {
        args: func_args![value: value!([1, 2, 3])],
        want: Ok(false),
    }

    float {
        args: func_args![value: 0.577],
        want: Ok(true),
    }
}

bench_function! {
    is_integer => vrl::stdlib::IsInteger;

    integer {
        args: func_args![value: 1701],
        want: Ok(true),
    }

    object {
        args: func_args![value: value!({"foo": "bar"})],
        want: Ok(false),
    }
}

bench_function! {
    is_ipv4 => vrl::stdlib::IsIpv4;

    not_string {
        args: func_args![value: 42],
        want: Ok(false),
    }

    ipv4 {
        args: func_args![value: "192.168.0.1"],
        want: Ok(true),
    }

    invalid_ipv4 {
        args: func_args![value: "192.168.0.299"],
        want: Ok(false),
    }

    ipv6 {
        args: func_args![value: "2404:6800:4003:c02::64"],
        want: Ok(false),
    }
}

bench_function! {
    is_ipv6 => vrl::stdlib::IsIpv6;

    not_string {
        args: func_args![value: 42],
        want: Ok(false),
    }

    ipv4 {
        args: func_args![value: "192.168.0.1"],
        want: Ok(false),
    }

    ipv6 {
        args: func_args![value: "2404:6800:4003:c02::64"],
        want: Ok(true),
    }

    invalid_ipv6 {
        args: func_args![value: "2404:6800:goat:c02::64"],
        want: Ok(false),
    }
}

bench_function! {
    is_json => vrl::stdlib::IsJson;

    map {
        args: func_args![value: r#"{"key": "value"}"#],
        want: Ok(true),
    }

    invalid_map {
        args: func_args![value: r#"{"key": "value""#],
        want: Ok(false),
    }

    exact_variant {
        args: func_args![value: r#"{"key": "value""#, variant: "object"],
        want: Ok(true),
    }
}

bench_function! {
    is_null => vrl::stdlib::IsNull;

    string {
        args: func_args![value: "foobar"],
        want: Ok(false),
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok(true),
    }
}

bench_function! {
    is_nullish => vrl::stdlib::IsNullish;

    whitespace {
        args: func_args![value: "         "],
        want: Ok(true),
    }

    hyphen {
        args: func_args![value: "-"],
        want: Ok(true),
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok(true),
    }

    not_empty {
        args: func_args![value: "foo"],
        want: Ok(false),
    }
}

bench_function! {
    is_object => vrl::stdlib::IsObject;

    integer {
        args: func_args![value: 1701],
        want: Ok(false),
    }

    object {
        args: func_args![value: value!({"foo": "bar"})],
        want: Ok(true),
    }
}

bench_function! {
    is_regex => vrl::stdlib::IsRegex;

    regex {
        args: func_args![value: value!(Regex::new(r"\d+").unwrap())],
        want: Ok(true),
    }

    object {
        args: func_args![value: value!({"foo": "bar"})],
        want: Ok(false),
    }
}

bench_function! {
    is_string => vrl::stdlib::IsString;

    string {
        args: func_args![value: "foobar"],
        want: Ok(true),
    }

    array {
        args: func_args![value: value!([1, 2, 3])],
        want: Ok(false),
    }
}

bench_function! {
    is_timestamp => vrl::stdlib::IsTimestamp;

    string {
        args: func_args![value: Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)],
        want: Ok(true),
    }

    array {
        args: func_args![value: value!([1, 2, 3])],
        want: Ok(false),
    }
}

bench_function! {
    join => vrl::stdlib::Join;

    literal {
        args: func_args![value: value!(["hello", "world"]), separator: " "],
        want: Ok("hello world"),
    }
}

bench_function! {
    keys => vrl::stdlib::Keys;

    literal {
        args: func_args![value: value!({"key1": "val1", "key2": "val2"})],
        want: Ok(value!(["key1", "key2"])),
    }
}

bench_function! {
    length => vrl::stdlib::Length;

    map {
        args: func_args![value: value!({foo: "bar", baz: true, baq: [1, 2, 3]})],
        want: Ok(3),
    }

    array {
        args: func_args![value: value!([1, 2, 3, 4, true, "hello"])],
        want: Ok(value!(6)),
    }

    string {
        args: func_args![value: "hello world"],
        want: Ok(value!(11))
    }
}

// TODO: Ensure tracing is enabled
bench_function! {
    log => vrl::stdlib::Log;

    literal {
        args: func_args![value: "ohno"],
        want: Ok(value!(null)),
    }
}

bench_function! {
    get => vrl::stdlib::Get;

    single {
        args: func_args![value: value!({ "foo": "bar" }), path: vec!["foo"]],
        want: Ok("bar"),
    }

    nested {
        args: func_args![value: value!({ "foo": { "bar": "baz" } }), path: vec!["foo", "bar"]],
        want: Ok("baz"),
    }

    indexing {
        args: func_args![value: value!([0, 42, 91]), path: vec![-2]],
        want: Ok(42),
    }
}

bench_function! {
    r#match => vrl::stdlib::Match;

    simple {
        args: func_args![value: "foo 2 bar", pattern: Regex::new("foo \\d bar").unwrap()],
        want: Ok(true),
    }
}

bench_function! {
    match_any => vrl::stdlib::MatchAny;

    simple {
        args: func_args![value: "foo 2 bar", patterns: vec![Regex::new(r"foo \d bar").unwrap()]],
        want: Ok(true),
    }
}

bench_function! {
    match_array => vrl::stdlib::MatchArray;

    single_match {
        args: func_args![
            value: value!(["foo 1 bar"]),
            pattern: Regex::new(r"foo \d bar").unwrap(),
        ],
        want: Ok(true),
    }

    no_match {
        args: func_args![
            value: value!(["foo x bar"]),
            pattern: Regex::new(r"foo \d bar").unwrap(),
        ],
        want: Ok(false),
    }

    some_match {
        args: func_args![
            value: value!(["foo 2 bar", "foo 3 bar", "foo 4 bar", "foo 5 bar"]),
            pattern: Regex::new(r"foo \d bar").unwrap(),
        ],
        want: Ok(true),
    }

    all_match {
        args: func_args![
            value: value!(["foo 2 bar", "foo 3 bar", "foo 4 bar", "foo 5 bar"]),
            pattern: Regex::new(r"foo \d bar").unwrap(),
            all: value!(true)
        ],
        want: Ok(true),
    }

    not_all_match {
        args: func_args![
            value: value!(["foo 2 bar", "foo 3 bar", "foo 4 bar", "foo x bar"]),
            pattern: Regex::new(r"foo \d bar").unwrap(),
            all: value!(true)
        ],
        want: Ok(false),
    }
}

bench_function! {
    match_datadog_query => vrl::stdlib::MatchDatadogQuery;

    equals_message {
        args: func_args![value: value!({"message": "match by word boundary"}), query: "match"],
        want: Ok(true),
    }

    equals_tag {
        args: func_args![value: value!({"tags": ["x:1", "y:2", "z:3"]}), query: "y:2"],
        want: Ok(true),
    }

    equals_facet {
        args: func_args![value: value!({"custom": {"z": 1}}), query: "@z:1"],
        want: Ok(true),
    }

    negate_wildcard_prefix_message {
        args: func_args![value: value!({"message": "vector"}), query: "-*tor"],
        want: Ok(false),
    }

    wildcard_prefix_tag_no_match {
        args: func_args![value: value!({"tags": ["b:vector"]}), query: "a:*tor"],
        want: Ok(false),
    }

    wildcard_suffix_facet {
        args: func_args![value: value!({"custom": {"a": "vector"}}), query: "@a:vec*"],
        want: Ok(true),
    }

    wildcard_multiple_message {
        args: func_args![value: value!({"message": "vector"}), query: "v*c*r"],
        want: Ok(true),
    }

    not_wildcard_multiple_facet_no_match {
        args: func_args![value: value!({"custom": {"b": "vector"}}), query: "NOT @a:v*c*r"],
        want: Ok(true),
    }

    negate_range_facet_between_no_match {
        args: func_args![value: value!({"custom": {"a": 200}}), query: "-@a:[1 TO 6]"],
        want: Ok(true),
    }

    not_range_facet_between_no_match_string {
        args: func_args![value: value!({"custom": {"a": "7"}}), query: r#"NOT @a:["1" TO "60"]"#],
        want: Ok(true),
    }

    exclusive_range_message_lower {
        args: func_args![value: value!({"message": "200"}), query: "{1 TO *}"],
        want: Ok(true),
    }

    not_exclusive_range_message_upper_no_match {
        args: func_args![value: value!({"message": "3"}), query: "NOT {* TO 3}"],
        want: Ok(true),
    }

    negate_message_and_or_2 {
        args: func_args![value: value!({"message": "this contains the_other"}), query: "this AND -(that OR the_other)"],
        want: Ok(false),
    }

    message_or_and {
        args: func_args![value: value!({"message": "just this"}), query: "this OR (that AND the_other)"],
        want: Ok(true),
    }

    kitchen_sink_2 {
        args: func_args![value: value!({"tags": ["c:that", "d:the_other"], "custom": {"b": "testing", "e": 3}}), query: "host:this OR ((@b:test* AND c:that) AND d:the_other @e:[1 TO 5])"],
        want: Ok(true),
    }
}

bench_function! {
    md5  => vrl::stdlib::Md5;

    literal {
        args: func_args![value: "foo"],
        want: Ok("acbd18db4cc2f85cedef654fccc4a4d8"),
    }
}

bench_function! {
    r#mod => vrl::stdlib::Mod;

    simple {
        args: func_args![
            value: value!(5),
            modulus: value!(2),
        ],
        want: Ok(value!(1))
    }
}

bench_function! {
    merge => vrl::stdlib::Merge;

    simple {
        args: func_args![
            to: value!({ "key1": "val1" }),
            from: value!({ "key2": "val2" }),
        ],
        want: Ok(value!({
            "key1": "val1",
            "key2": "val2",
        }))
    }

    shallow {
        args: func_args![
            to: value!({
                "key1": "val1",
                "child": { "grandchild1": "val1" },
            }),
            from: value!({
                "key2": "val2",
                "child": { "grandchild2": "val2" },
            })
        ],
        want: Ok(value!({
            "key1": "val1",
            "key2": "val2",
            "child": { "grandchild2": "val2" },
        }))
    }

    deep {
        args: func_args![
            to: value!({
                "key1": "val1",
                "child": { "grandchild1": "val1" },
            }),
            from: value!({
                "key2": "val2",
                "child": { "grandchild2": "val2" },
            }),
            deep: true
        ],
        want: Ok(value!({
            "key1": "val1",
            "key2": "val2",
            "child": {
                "grandchild1": "val1",
                "grandchild2": "val2",
            },
        }))
    }
}

bench_function! {
    object => vrl::stdlib::Object;

    object {
        args: func_args![value: value!({"foo": "bar"})],
        want: Ok(value!({"foo": "bar"})),
    }
}

bench_function! {
    parse_aws_alb_log => vrl::stdlib::ParseAwsAlbLog;

    literal {
        args: func_args![
            value: r#"http 2018-07-02T22:23:00.186641Z app/my-loadbalancer/50dc6c495c0c9188 192.168.131.39:2817 10.0.0.1:80 0.000 0.001 0.000 200 200 34 366 "GET http://www.example.com:80/ HTTP/1.1" "curl/7.46.0" - - arn:aws:elasticloadbalancing:us-east-2:123456789012:targetgroup/my-targets/73e2d6bc24d8a067 "Root=1-58337262-36d228ad5d99923122bbe354" "-" "-" 0 2018-07-02T22:22:48.364000Z "forward" "-" "-" 10.0.0.1:80 200 "-" "-""#,
        ],
        want: Ok(value!({
            "actions_executed": "forward",
            "chosen_cert_arn": null,
            "classification": null,
            "classification_reason": null,
            "client_host": "192.168.131.39:2817",
            "domain_name": null,
            "elb": "app/my-loadbalancer/50dc6c495c0c9188",
            "elb_status_code": "200",
            "error_reason": null,
            "matched_rule_priority": "0",
            "received_bytes": 34,
            "redirect_url": null,
            "request_creation_time": "2018-07-02T22:22:48.364000Z",
            "request_method": "GET",
            "request_processing_time": 0.0,
            "request_protocol": "HTTP/1.1",
            "request_url": "http://www.example.com:80/",
            "response_processing_time": 0.0,
            "sent_bytes": 366,
            "ssl_cipher": null,
            "ssl_protocol": null,
            "target_group_arn": "arn:aws:elasticloadbalancing:us-east-2:123456789012:targetgroup/my-targets/73e2d6bc24d8a067",
            "target_host": "10.0.0.1:80",
            "target_port_list": ["10.0.0.1:80"],
            "target_processing_time": 0.001,
            "target_status_code": "200",
            "target_status_code_list": ["200"],
            "timestamp": "2018-07-02T22:23:00.186641Z",
            "trace_id": "Root=1-58337262-36d228ad5d99923122bbe354",
            "type": "http",
            "user_agent": "curl/7.46.0"
        })),
    }
}

bench_function! {
    parse_aws_cloudwatch_log_subscription_message => vrl::stdlib::ParseAwsCloudWatchLogSubscriptionMessage;

    literal {
        args: func_args![value: r#"
{
    "messageType": "DATA_MESSAGE",
    "owner": "071959437513",
    "logGroup": "/jesse/test",
    "logStream": "test",
    "subscriptionFilters": [
    "Destination"
    ],
    "logEvents": [
    {
        "id": "35683658089614582423604394983260738922885519999578275840",
        "timestamp": 1600110569039,
        "message": "{\"bytes\":26780,\"datetime\":\"14/Sep/2020:11:45:41 -0400\",\"host\":\"157.130.216.193\",\"method\":\"PUT\",\"protocol\":\"HTTP/1.0\",\"referer\":\"https://www.principalcross-platform.io/markets/ubiquitous\",\"request\":\"/expedite/convergence\",\"source_type\":\"stdin\",\"status\":301,\"user-identifier\":\"-\"}"
    },
    {
        "id": "35683658089659183914001456229543810359430816722590236673",
        "timestamp": 1600110569041,
        "message": "{\"bytes\":17707,\"datetime\":\"14/Sep/2020:11:45:41 -0400\",\"host\":\"109.81.244.252\",\"method\":\"GET\",\"protocol\":\"HTTP/2.0\",\"referer\":\"http://www.investormission-critical.io/24/7/vortals\",\"request\":\"/scale/functionalities/optimize\",\"source_type\":\"stdin\",\"status\":502,\"user-identifier\":\"feeney1708\"}"
    }
    ]
}
"#],
        want: Ok(value!({
            "owner":  "071959437513",
            "message_type":  "DATA_MESSAGE",
            "log_group":  "/jesse/test",
            "log_stream":  "test",
            "subscription_filters":  ["Destination"],
            "log_events": [{
                "id":  "35683658089614582423604394983260738922885519999578275840",
                "timestamp":  (Utc.timestamp_opt(1600110569, 39000000).single().expect("invalid timestamp")),
                "message":  r#"{"bytes":26780,"datetime":"14/Sep/2020:11:45:41 -0400","host":"157.130.216.193","method":"PUT","protocol":"HTTP/1.0","referer":"https://www.principalcross-platform.io/markets/ubiquitous","request":"/expedite/convergence","source_type":"stdin","status":301,"user-identifier":"-"}"#,
            }, {
                "id":  "35683658089659183914001456229543810359430816722590236673",
                "timestamp":  (Utc.timestamp_opt(1600110569, 41000000).single().expect("invalid timestamp")),
                "message":  r#"{"bytes":17707,"datetime":"14/Sep/2020:11:45:41 -0400","host":"109.81.244.252","method":"GET","protocol":"HTTP/2.0","referer":"http://www.investormission-critical.io/24/7/vortals","request":"/scale/functionalities/optimize","source_type":"stdin","status":502,"user-identifier":"feeney1708"}"#,
            }]
        }))
    }
}

bench_function! {
    parse_aws_vpc_flow_log => vrl::stdlib::ParseAwsVpcFlowLog;

    literal {
        args: func_args![
            value:"3 eni-33333333333333333 123456789010 vpc-abcdefab012345678 subnet-22222222bbbbbbbbb i-01234567890123456 10.20.33.164 10.40.2.236 39812 80 6 3 IPv4 10.20.33.164 10.40.2.236 ACCEPT OK",
            format: "version interface_id account_id vpc_id subnet_id instance_id srcaddr dstaddr srcport dstport protocol tcp_flags type pkt_srcaddr pkt_dstaddr action log_status",
        ],
        want: Ok(value!({
            "account_id": 123456789010i64,
            "action": "ACCEPT",
            "dstaddr": "10.40.2.236",
            "dstport": 80,
            "instance_id": "i-01234567890123456",
            "interface_id": "eni-33333333333333333",
            "log_status": "OK",
            "pkt_dstaddr": "10.40.2.236",
            "pkt_srcaddr": "10.20.33.164",
            "protocol": 6,
            "srcaddr": "10.20.33.164",
            "srcport": 39812,
            "subnet_id": "subnet-22222222bbbbbbbbb",
            "tcp_flags": 3,
            "type": "IPv4",
            "version": 3,
            "vpc_id": "vpc-abcdefab012345678",
        })),
    }
}

bench_function! {
    parse_cef => vrl::stdlib::ParseCef;

    simple {
        args: func_args! [
            value: r#"CEF:0|CyberArk|PTA|12.6|1|Suspected credentials theft|8|suser=mike2@prod1.domain.com shost=prod1.domain.com src=1.1.1.1"#
        ],
        want: Ok(value!({
            "cefVersion": "0",
            "deviceVendor": "CyberArk",
            "deviceProduct": "PTA",
            "deviceVersion": "12.6",
            "deviceEventClassId": "1",
            "name": "Suspected credentials theft",
            "severity": "8",
            "suser": "mike2@prod1.domain.com",
            "shost": "prod1.domain.com",
            "src": "1.1.1.1"
        }))
    }

    complex {
        args: func_args! [
            value: r"CEF:0|Check Point|VPN-1 & FireWall-1|Check Point|Log|https|Unknown|act=Accept destinationTranslatedAddress=0.0.0.0 destinationTranslatedPort=0 deviceDirection=0 rt=1543270652000 sourceTranslatedAddress=192.168.103.254 sourceTranslatedPort=35398 spt=49363 dpt=443 cs2Label=Rule Name layer_name=Network layer_uuid=b406b732-2437-4848-9741-6eae1f5bf112 match_id=4 parent_rule=0 rule_action=Accept rule_uid=9e5e6e74-aa9a-4693-b9fe-53712dd27bea ifname=eth0 logid=0 loguid={0x5bfc70fc,0x1,0xfe65a8c0,0xc0000001} origin=192.168.101.254 originsicname=CN\=R80,O\=R80_M..6u6bdo sequencenum=1 version=5 dst=52.173.84.157 inzone=Internal nat_addtnl_rulenum=1 nat_rulenum=4 outzone=External product=VPN-1 & FireWall-1 proto=6 service_id=https src=192.168.101.100",
        ],
        want: Ok(value!({
            "cefVersion":"0",
            "deviceVendor":"Check Point",
            "deviceProduct":"VPN-1 & FireWall-1",
            "deviceVersion":"Check Point",
            "deviceEventClassId":"Log",
            "name":"https",
            "severity":"Unknown",
            "act": "Accept",
            "destinationTranslatedAddress": "0.0.0.0",
            "destinationTranslatedPort": "0",
            "deviceDirection": "0",
            "rt": "1543270652000",
            "sourceTranslatedAddress": "192.168.103.254",
            "sourceTranslatedPort": "35398",
            "spt": "49363",
            "dpt": "443",
            "cs2Label": "Rule Name",
            "layer_name": "Network",
            "layer_uuid": "b406b732-2437-4848-9741-6eae1f5bf112",
            "match_id": "4",
            "parent_rule": "0",
            "rule_action": "Accept",
            "rule_uid": "9e5e6e74-aa9a-4693-b9fe-53712dd27bea",
            "ifname": "eth0",
            "logid": "0",
            "loguid": "{0x5bfc70fc,0x1,0xfe65a8c0,0xc0000001}",
            "origin": "192.168.101.254",
            "originsicname": "CN=R80,O=R80_M..6u6bdo",
            "sequencenum": "1",
            "version": "5",
            "dst": "52.173.84.157",
            "inzone": "Internal",
            "nat_addtnl_rulenum": "1",
            "nat_rulenum": "4",
            "outzone": "External",
            "product": "VPN-1 & FireWall-1",
            "proto": "6",
            "service_id": "https",
            "src": "192.168.101.100",
        }))
    }
}

bench_function! {
    parse_apache_log => vrl::stdlib::ParseApacheLog;

    common {
        args: func_args![value: r#"127.0.0.1 bob frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#,
                         format: "common"
        ],
        want: Ok(value!({
            "host": "127.0.0.1",
            "identity": "bob",
            "user": "frank",
            "timestamp": (DateTime::parse_from_rfc3339("2000-10-10T20:55:36Z").unwrap().with_timezone(&Utc)),
            "message": "GET /apache_pb.gif HTTP/1.0",
            "method": "GET",
            "path": "/apache_pb.gif",
            "protocol": "HTTP/1.0",
            "status": 200,
            "size": 2326,
        })),
    }

    combined {
        args: func_args![value: r#"127.0.0.1 bob frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326 "http://www.seniorinfomediaries.com/vertical/channels/front-end/bandwidth" "Mozilla/5.0 (X11; Linux i686; rv:5.0) Gecko/1945-10-12 Firefox/37.0""#,
                         format: "combined"
        ],
        want: Ok(value!({
            "agent": "Mozilla/5.0 (X11; Linux i686; rv:5.0) Gecko/1945-10-12 Firefox/37.0",
            "host": "127.0.0.1",
            "identity": "bob",
            "user": "frank",
            "referrer": "http://www.seniorinfomediaries.com/vertical/channels/front-end/bandwidth",
            "timestamp": (DateTime::parse_from_rfc3339("2000-10-10T20:55:36Z").unwrap().with_timezone(&Utc)),
            "message": "GET /apache_pb.gif HTTP/1.0",
            "method": "GET",
            "path": "/apache_pb.gif",
            "protocol": "HTTP/1.0",
            "status": 200,
            "size": 2326,
        })),
    }

    error {
        args: func_args![value: r#"[01/Mar/2021:12:00:19 +0000] [ab:alert] [pid 4803:tid 3814] [client 147.159.108.175:24259] I will bypass the haptic COM bandwidth, that should matrix the CSS driver!"#,
                         format: "error"
        ],
        want: Ok(value!({
            "client": "147.159.108.175",
            "message": "I will bypass the haptic COM bandwidth, that should matrix the CSS driver!",
            "module": "ab",
            "pid": 4803,
            "port": 24259,
            "severity": "alert",
            "thread": "3814",
            "timestamp": (DateTime::parse_from_rfc3339("2021-03-01T12:00:19Z").unwrap().with_timezone(&Utc)),
        })),
    }
}

bench_function! {
    parse_common_log => vrl::stdlib::ParseCommonLog;

    literal {
        args: func_args![value: r#"127.0.0.1 bob frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#],
        want: Ok(value!({
            "host": "127.0.0.1",
            "identity": "bob",
            "user": "frank",
            "timestamp": (DateTime::parse_from_rfc3339("2000-10-10T20:55:36Z").unwrap().with_timezone(&Utc)),
            "message": "GET /apache_pb.gif HTTP/1.0",
            "method": "GET",
            "path": "/apache_pb.gif",
            "protocol": "HTTP/1.0",
            "status": 200,
            "size": 2326,
        })),
    }
}

bench_function! {
    parse_csv => vrl::stdlib::ParseCsv;

    literal {
        args: func_args![value: "foo,bar"],
        want: Ok(value!(["foo","bar"]))
    }
}

bench_function! {
    parse_duration => vrl::stdlib::ParseDuration;

    literal {
        args: func_args![value: "1005ms", unit: "s"],
        want: Ok(1.005),
    }
}

bench_function! {
    parse_glog  => vrl::stdlib::ParseGlog;

    literal {
        args: func_args![value: "I20210131 14:48:54.411655 15520 main.c++:9] Hello world!"],
        want: Ok(value!({
            "level": "info",
            "timestamp": (DateTime::parse_from_rfc3339("2021-01-31T14:48:54.411655Z").unwrap().with_timezone(&Utc)),
            "id": 15520,
            "file": "main.c++",
            "line": 9,
            "message": "Hello world!",
        })),
    }
}

bench_function! {
    parse_grok => vrl::stdlib::ParseGrok;

    simple {
        args: func_args![
            value: "2020-10-02T23:22:12.223222Z info Hello world",
            pattern: "%{TIMESTAMP_ISO8601:timestamp} %{LOGLEVEL:level} %{GREEDYDATA:message}",
        ],
        want: Ok(value!({
            "timestamp": "2020-10-02T23:22:12.223222Z",
            "level": "info",
            "message": "Hello world",
        })),
    }
}

bench_function! {
    parse_groks => vrl::stdlib::ParseGroks;

    simple {
        args: func_args![
            value: r##"2020-10-02T23:22:12.223222Z info hello world"##,
            patterns: Value::Array(vec![
                "%{common_prefix} %{_status} %{_message}".into(),
                "%{common_prefix} %{_message}".into(),
                ]),
            aliases: value!({
                common_prefix: "%{_timestamp} %{_loglevel}",
                _timestamp: "%{TIMESTAMP_ISO8601:timestamp}",
                _loglevel: "%{LOGLEVEL:level}",
                _status: "%{POSINT:status}",
                _message: "%{GREEDYDATA:message}"
            })
        ],
        want: Ok(Value::from(btreemap! {
            "timestamp" => "2020-10-02T23:22:12.223222Z",
            "level" => "info",
            "message" => "hello world"
        }))
    }
}

bench_function! {
    parse_int => vrl::stdlib::ParseInt;

    decimal {
        args: func_args![value: "-42"],
        want: Ok(-42),
    }

    hexidecimal {
        args: func_args![value: "0x2a"],
        want: Ok(42),
    }

    explicit_hexidecimal {
        args: func_args![value: "2a", base: 16],
        want: Ok(42),
    }
}

bench_function! {
    parse_json => vrl::stdlib::ParseJson;

    map {
        args: func_args![value: r#"{"key": "value"}"#],
        want: Ok(value!({key: "value"})),
    }

    map_max_depth {
        args: func_args![value: r#"{"key": "value"}"#, max_depth: 10],
        want: Ok(value!({key: "value"})),
    }

    nested {
        args: func_args![value: r#"{"1":{"2":{"3":{"4":{"5":{"6":"end"}}}}}}"#],
        want: Ok(value!({"1":{"2":{"3":{"4":{"5":{"6":"end"}}}}}})),
    }

    nested_max_depth_1 {
        args: func_args![value: r#"{"1":{"2":{"3":{"4":{"5":{"6":"end"}}}}}}"#, max_depth: 1],
        want: Ok(value!({"1":"{\"2\":{\"3\":{\"4\":{\"5\":{\"6\":\"end\"}}}}}"})),
    }

    nested_max_depth_10 {
        args: func_args![value: r#"{"1":{"2":{"3":{"4":{"5":{"6":"end"}}}}}}"#, max_depth: 10],
        want: Ok(value!({"1":{"2":{"3":{"4":{"5":{"6":"end"}}}}}})),
    }
}

bench_function! {
    parse_key_value => vrl::stdlib::ParseKeyValue;

    logfmt {
        args: func_args! [
            value: r#"level=info msg="Stopping all fetchers" tag=stopping_fetchers id=ConsumerFetcherManager-1382721708341 module=kafka.consumer.ConsumerFetcherManager"#
        ],
        want: Ok(value!({
            level: "info",
            msg: "Stopping all fetchers",
            tag: "stopping_fetchers",
            id: "ConsumerFetcherManager-1382721708341",
            module: "kafka.consumer.ConsumerFetcherManager"
        }))
    }

    standalone_key_disabled {
        args: func_args! [
            value: r#"level=info msg="Stopping all fetchers" tag=stopping_fetchers id=ConsumerFetcherManager-1382721708341 module=kafka.consumer.ConsumerFetcherManager"#,
            accept_standalone_key: false
        ],
        want: Ok(value!({
            level: "info",
            msg: "Stopping all fetchers",
            tag: "stopping_fetchers",
            id: "ConsumerFetcherManager-1382721708341",
            module: "kafka.consumer.ConsumerFetcherManager"
        }))
    }
}

bench_function! {
    parse_klog  => vrl::stdlib::ParseKlog;

    literal {
        args: func_args![value: "I0505 17:59:40.692994   28133 klog.go:70] hello from klog"],
        want: Ok(btreemap! {
            "level" => "info",
            "timestamp" => Value::Timestamp(DateTime::parse_from_rfc3339(&format!("{}-05-05T17:59:40.692994Z", Utc::now().year())).unwrap().into()),
            "id" => 28133,
            "file" => "klog.go",
            "line" => 70,
            "message" => "hello from klog",
        }),
    }
}

bench_function! {
    parse_nginx_log => vrl::stdlib::ParseNginxLog;

    combined {
        args: func_args![
            value: r#"172.17.0.1 alice - [01/Apr/2021:12:02:31 +0000] "POST /not-found HTTP/1.1" 404 153 "http://localhost/somewhere" "Mozilla/5.0 (Windows NT 6.1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/72.0.3626.119 Safari/537.36" "2.75""#,
            format: "combined",
        ],
        want: Ok(value!({
            "client": "172.17.0.1",
            "user": "alice",
            "timestamp": (DateTime::parse_from_rfc3339("2021-04-01T12:02:31Z").unwrap().with_timezone(&Utc)),
            "request": "POST /not-found HTTP/1.1",
            "method": "POST",
            "path": "/not-found",
            "protocol": "HTTP/1.1",
            "status": 404,
            "size": 153,
            "referer": "http://localhost/somewhere",
            "agent": "Mozilla/5.0 (Windows NT 6.1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/72.0.3626.119 Safari/537.36",
            "compression": "2.75",
        })),
    }

    ingress_upstreaminfo {
        args: func_args![
            value: r#"0.0.0.0 - - [18/Mar/2023:15:00:00 +0000] "GET /some/path HTTP/2.0" 200 12312 "https://10.0.0.1/some/referer" "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Safari/537.36" 462 0.050 [some-upstream-service-9000] [] 10.0.50.80:9000 19437 0.049 200 752178adb17130b291aefd8c386279e7"#,
            format: "ingress_upstreaminfo",
        ],
        want: Ok(value!({
            "remote_addr": "0.0.0.0",
            "timestamp": (DateTime::parse_from_rfc3339("2023-03-18T15:00:00Z").unwrap().with_timezone(&Utc)),
            "request": "GET /some/path HTTP/2.0",
            "method": "GET",
            "path": "/some/path",
            "protocol": "HTTP/2.0",
            "status": 200,
            "body_bytes_size": 12312,
            "http_referer": "https://10.0.0.1/some/referer",
            "http_user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Safari/537.36",
            "request_length": 462,
            "request_time": 0.050,
            "proxy_upstream_name": "some-upstream-service-9000",
            "upstream_addr": "10.0.50.80:9000",
            "upstream_response_length": 19437,
            "upstream_response_time": 0.049,
            "upstream_status": 200,
            "req_id": "752178adb17130b291aefd8c386279e7",
        })),
    }

    error {
        args: func_args![value: r#"2021/04/01 13:02:31 [error] 31#31: *1 open() "/usr/share/nginx/html/not-found" failed (2: No such file or directory), client: 172.17.0.1, server: localhost, request: "POST /not-found HTTP/1.1", host: "localhost:8081""#,
                         format: "error"
        ],
        want: Ok(value!({
            "timestamp": (DateTime::parse_from_rfc3339("2021-04-01T13:02:31Z").unwrap().with_timezone(&Utc)),
            "severity": "error",
            "pid": 31,
            "tid": 31,
            "cid": 1,
            "message": "open() \"/usr/share/nginx/html/not-found\" failed (2: No such file or directory)",
            "client": "172.17.0.1",
            "server": "localhost",
            "request": "POST /not-found HTTP/1.1",
            "host": "localhost:8081",
        })),
    }
}

bench_function! {
    parse_query_string => vrl::stdlib::ParseQueryString;

    literal {
        args: func_args![value: "foo=%2B1&bar=2"],
        want: Ok(value!({
            foo: "+1",
            bar: "2",
        }))
    }
}

bench_function! {
    parse_regex => vrl::stdlib::ParseRegex;

    matches {
        args: func_args! [
            value: "5.86.210.12 - zieme4647 5667 [19/06/2019:17:20:49 -0400] \"GET /embrace/supply-chains/dynamic/vertical\" 201 20574",
            pattern: Regex::new(r#"^(?P<host>[\w\.]+) - (?P<user>[\w]+) (?P<bytes_in>[\d]+) \[(?P<timestamp>.*)\] "(?P<method>[\w]+) (?P<path>.*)" (?P<status>[\d]+) (?P<bytes_out>[\d]+)$"#)
                .unwrap(),
            numeric_groups: true
        ],
        want: Ok(value!({
            "bytes_in": "5667",
            "host": "5.86.210.12",
            "user": "zieme4647",
            "timestamp": "19/06/2019:17:20:49 -0400",
            "method": "GET",
            "path": "/embrace/supply-chains/dynamic/vertical",
            "status": "201",
            "bytes_out": "20574",
            "0": "5.86.210.12 - zieme4647 5667 [19/06/2019:17:20:49 -0400] \"GET /embrace/supply-chains/dynamic/vertical\" 201 20574",
            "1": "5.86.210.12",
            "2": "zieme4647",
            "3": "5667",
            "4": "19/06/2019:17:20:49 -0400",
            "5": "GET",
            "6": "/embrace/supply-chains/dynamic/vertical",
            "7": "201",
            "8": "20574",
        }))
    }

    single_match {
        args: func_args! [
            value: "first group and second group",
            pattern: Regex::new(r#"(?P<number>.*?) group"#).unwrap()
        ],
        want: Ok(value!({
            "number": "first",
        }))
    }
}

bench_function! {
    parse_regex_all => vrl::stdlib::ParseRegexAll;

    matches {
        args: func_args![
            value: "apples and carrots, peaches and peas",
            pattern: Regex::new(r"(?P<fruit>[\w\.]+) and (?P<veg>[\w]+)").unwrap(),
            numeric_groups: true
        ],
        want: Ok(value!([
                {
                    "fruit": "apples",
                    "veg": "carrots",
                    "0": "apples and carrots",
                    "1": "apples",
                    "2": "carrots"
                },
                {
                    "fruit": "peaches",
                    "veg": "peas",
                    "0": "peaches and peas",
                    "1": "peaches",
                    "2": "peas"
                }]))
    }
}

bench_function! {
    parse_ruby_hash => vrl::stdlib::ParseRubyHash;

    matches {
        args: func_args![
            value: r#"{ "test" => "value", "testNum" => 0.2, "testObj" => { "testBool" => true } }"#,
        ],
        want: Ok(value!({
            test: "value",
            testNum: 0.2,
            testObj: {
                testBool: true,
            }
        }))
    }
}

bench_function! {
    parse_syslog => vrl::stdlib::ParseSyslog;

    rfc3164 {
        args: func_args![
            value: r#"<190>Dec 28 2020 16:49:07 plertrood-thinkpad-x220 nginx: 127.0.0.1 - - [28/Dec/2019:16:49:07 +0000] "GET / HTTP/1.1" 304 0 "-" "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:71.0) Gecko/20100101 Firefox/71.0""#
        ],
        want: Ok(value!({
            "severity": "info",
            "facility": "local7",
            "timestamp": (Utc.ymd(2020, 12, 28).and_hms_milli(16, 49, 7, 0)),
            "hostname": "plertrood-thinkpad-x220",
            "appname": "nginx",
            "message": r#"127.0.0.1 - - [28/Dec/2019:16:49:07 +0000] "GET / HTTP/1.1" 304 0 "-" "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:71.0) Gecko/20100101 Firefox/71.0""#,
        }))
    }

    rfc5424 {
        args: func_args![value: r#"<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut="3" eventSource= "Application" eventID="1011"] Try to override the THX port, maybe it will reboot the neural interface!"#],
        want: Ok(value!({
            "severity": "notice",
            "facility": "user",
            "timestamp": (Utc.ymd(2020, 3, 13).and_hms_milli(20, 45, 38, 119)),
            "hostname": "dynamicwireless.name",
            "appname": "non",
            "procid": 2426,
            "msgid": "ID931",
            "exampleSDID@32473": {
                "iut": "3",
                "eventSource": "Application",
                "eventID": "1011",
            },
            "message": "Try to override the THX port, maybe it will reboot the neural interface!",
            "version": 1,
        }))
    }
}

bench_function! {
    parse_timestamp => vrl::stdlib::ParseTimestamp;

    rfc3339 {
        args: func_args![value: "2001-07-08T00:34:60.026490+09:30", format: "%+"],
        want: Ok(DateTime::parse_from_rfc3339("2001-07-08T00:34:60.026490+09:30").unwrap().with_timezone(&Utc))
    }

    rfc2822 {
        args: func_args![value: "Wed, 16 Oct 2019 12:00:00 +0000", format: "%a, %e %b %Y %T %z"],
        want: Ok(DateTime::parse_from_rfc2822("Wed, 16 Oct 2019 12:00:00 +0000").unwrap().with_timezone(&Utc))
    }
}

bench_function! {
    parse_tokens => vrl::stdlib::ParseTokens;

    literal {
        args: func_args![value: "217.250.207.207 - - [07/Sep/2020:16:38:00 -0400] \"DELETE /deliverables/next-generation/user-centric HTTP/1.1\" 205 11881"],
        want: Ok(value!([
            "217.250.207.207",
            null,
            null,
            "07/Sep/2020:16:38:00 -0400",
            "DELETE /deliverables/next-generation/user-centric HTTP/1.1",
            "205",
            "11881",
        ])),
    }
}

bench_function! {
    parse_url => vrl::stdlib::ParseUrl;

    literal {
        args: func_args![value: "https://vector.dev"],
        want: Ok(value!({
                        "scheme": "https",
                        "username": "",
                        "password": "",
                        "host": "vector.dev",
                        "port": null,
                        "path": "/",
                        "query": {},
                        "fragment": null,
        }))
    }
}

bench_function! {
    parse_user_agent => vrl::stdlib::ParseUserAgent;

    fast {
        args: func_args![value: "Mozilla Firefox 1.0.1 Mozilla/5.0 (X11; U; Linux i686; de-DE; rv:1.7.6) Gecko/20050223 Firefox/1.0.1"],
        want: Ok(value!({
            "browser": {
                "family": "Firefox",
                "version": "1.0.1",
            },
            "device": {
                "category": "pc",
            },
            "os": {
                "family": "Linux",
                "version": null,
            },
        }))
    }

    enriched {
        args: func_args![value: "Opera/9.80 (J2ME/MIDP; Opera Mini/4.3.24214; iPhone; CPU iPhone OS 4_2_1 like Mac OS X; AppleWebKit/24.783; U; en) Presto/2.5.25 Version/10.54", mode: "enriched"],
        want: Ok(value!({
            "browser": {
                "family": "Opera Mini",
                "major": "4",
                "minor": "3",
                "patch": "24214",
                "version": "10.54",
            },
            "device": {
                "brand": "Apple",
                "category": "smartphone",
                "family": "iPhone",
                "model": "iPhone",
            },
            "os": {
                "family": "iOS",
                "major": "4",
                "minor": "2",
                "patch": "1",
                "patch_minor": null,
                "version": "4.2.1",
            },
        }))
    }
}

bench_function! {
    parse_xml => vrl::stdlib::ParseXml;

    simple_text {
        args: func_args![ value: r#"<a>test</a>"# ],
        want: Ok(value!({ "a": "test" }))
    }

    include_attr {
        args: func_args![ value: r#"<a href="https://vector.dev">test</a>"# ],
        want: Ok(value!({ "a": { "@href": "https://vector.dev", "text": "test" } }))
    }

    exclude_attr {
        args: func_args![ value: r#"<a href="https://vector.dev">test</a>"#, include_attr: false ],
        want: Ok(value!({ "a": "test" }))
    }

    custom_text_key {
        args: func_args![ value: r#"<b>test</b>"#, text_key: "node", always_use_text_key: true ],
        want: Ok(value!({ "b": { "node": "test" } }))
    }

    nested_object {
        args: func_args![ value: r#"<a><b>one</b><c>two</c></a>"# ],
        want: Ok(value!({ "a": { "b": "one", "c": "two" } }))
    }

    nested_object_array {
        args: func_args![ value: r#"<a><b>one</b><b>two</b></a>"# ],
        want: Ok(value!({ "a": { "b": ["one", "two"] } }))
    }

    header_and_comments {
        args: func_args![ value: indoc!{r#"
            <?xml version="1.0" encoding="ISO-8859-1"?>
            <!-- Example found somewhere in the deep depths of the web -->
            <note>
                <to>Tove</to>
                <!-- Randomly inserted inner comment -->
                <from>Jani</from>
                <heading>Reminder</heading>
                <body>Don't forget me this weekend!</body>
            </note>

            <!-- Could literally be placed anywhere -->
        "#}],
        want: Ok(value!(
            {
                "note": {
                    "to": "Tove",
                    "from": "Jani",
                    "heading": "Reminder",
                    "body": "Don't forget me this weekend!"
                }
            }
        ))
    }

    mixed_types {
        args: func_args![ value: indoc!{r#"
            <?xml version="1.0" encoding="ISO-8859-1"?>
            <!-- Mixed types -->
            <data>
                <!-- Booleans -->
                <item>true</item>
                <item>false</item>
                <!-- String -->
                <item>string!</item>
                <!-- Empty object -->
                <item />
                <!-- Literal value "null" -->
                <item>null</item>
                <!-- Integer -->
                <item>1</item>
                <!-- Float -->
                <item>1.0</item>
            </data>
        "#}],
        want: Ok(value!(
            {
                "data": {
                    "item": [
                        true,
                        false,
                        "string!",
                        {},
                        null,
                        1,
                        1.0
                    ]
                }
            }
        ))
    }

    just_strings {
        args: func_args![ value: indoc!{r#"
            <?xml version="1.0" encoding="ISO-8859-1"?>
            <!-- All scalar types are just strings -->
            <data>
                <item>true</item>
                <item>false</item>
                <item>string!</item>
                <!-- Still an empty object -->
                <item />
                <item>null</item>
                <item>1</item>
                <item>1.0</item>
            </data>
        "#}, parse_null: false, parse_bool: false, parse_number: false],
        want: Ok(value!(
            {
                "data": {
                    "item": [
                        "true",
                        "false",
                        "string!",
                        {},
                        "null",
                        "1",
                        "1.0"
                    ]
                }
            }
        ))
    }

    untrimmed {
        args: func_args![ value: "<root>  <a>test</a>  </root>", trim: false ],
        want: Ok(value!(
            {
                "root": {
                    "a": "test",
                    "text": ["  ", "  "],
                }
            }
        ))
    }

    invalid_token {
        args: func_args![ value: "true" ],
        want: Err("unable to parse xml: unknown token at 1:1")
    }
}

bench_function! {
    push => vrl::stdlib::Push;

    literal {
        args: func_args![value: value!([11, false, 42.5]), item: "foo"],
        want: Ok(value!([11, false, 42.5, "foo"])),
    }
}

bench_function! {
    random_int => vrl::stdlib::RandomInt;

    literal {
        args: func_args![min: 1, max: 2],
        want: Ok(1),
    }
}

bench_function! {
    redact => vrl::stdlib::Redact;

    regex {
        args: func_args![
            value: "hello 123456 world",
            filters: vec![Regex::new(r"\d+").unwrap()],
        ],
        want: Ok("hello [REDACTED] world"),
    }

    us_social_security_number {
        args: func_args![
            value: "hello 123-12-1234 world",
            filters: vec!["us_social_security_number"],
        ],
        want: Ok("hello [REDACTED] world"),
    }
}

bench_function! {
    remove => vrl::stdlib::Remove;

    single {
        args: func_args![value: value!({ "foo": "bar", "baz": true }), path: vec!["foo"]],
        want: Ok(value!({ "baz": true })),
    }

    nested {
        args: func_args![value: value!({ "foo": { "bar": "baz" } }), path: vec!["foo", "bar"]],
        want: Ok(value!({ "foo": {} })),
    }

    indexing {
        args: func_args![value: value!([0, 42, 91]), path: vec![-2]],
        want: Ok(vec![0, 91]),
    }
}

bench_function! {
    replace => vrl::stdlib::Replace;

    string {
        args: func_args![
            value: "I like apples and bananas",
            pattern: "a",
            with: "o",
        ],
        want: Ok("I like opples ond bononos")
    }

    regex {
        args: func_args![
            value: "I like apples and bananas",
            pattern: Regex::new("[a]").unwrap(),
            with: "o",
        ],
        want: Ok("I like opples ond bononos")
    }
}

bench_function! {
    reverse_dns => vrl::stdlib::ReverseDns;

    google {
        args: func_args![value: value!("8.8.8.8")],
        want: Ok(value!("dns.google")),
    }
}

bench_function! {
    round => vrl::stdlib::Round;

    integer {
        args: func_args![value: 1234.56789, precision: 4],
        want: Ok(1234.5679)
    }

    float {
        args: func_args![value: 1234, precision: 4],
        want: Ok(1234)
    }
}

bench_function! {
    sha1 => vrl::stdlib::Sha1;

    literal {
        args: func_args![value: "foo"],
        want: Ok("0beec7b5ea3f0fdbc95d0dd47f3c5bc275da8a33")
    }
}

bench_function! {
    sha2 => vrl::stdlib::Sha2;

    default {
        args: func_args![value: "foo"],
        want: Ok("d58042e6aa5a335e03ad576c6a9e43b41591bfd2077f72dec9df7930e492055d")
    }
}

bench_function! {
    sha3 => vrl::stdlib::Sha3;

    default {
        args: func_args![value: "foo"],
        want: Ok("4bca2b137edc580fe50a88983ef860ebaca36c857b1f492839d6d7392452a63c82cbebc68e3b70a2a1480b4bb5d437a7cba6ecf9d89f9ff3ccd14cd6146ea7e7")
    }
}

bench_function! {
    slice => vrl::stdlib::Slice;

    literal {
        args: func_args![
            value: "Supercalifragilisticexpialidocious",
            start: 5,
            end: 9,
        ],
        want: Ok("cali")
    }
}

bench_function! {
    split => vrl::stdlib::Split;

    string {
        args: func_args![value: "foo,bar,baz", pattern: ","],
        want: Ok(value!(["foo", "bar", "baz"]))
    }

    regex {
        args: func_args![value: "foo,bar,baz", pattern: Regex::new("[,]").unwrap()],
        want: Ok(value!(["foo", "bar", "baz"]))
    }
}

bench_function! {
    starts_with  => vrl::stdlib::StartsWith;

    case_sensitive {
        args: func_args![value: "abcdefg", substring: "abc", case_sensitive: true],
        want: Ok(value!(true)),
    }

    case_insensitive {
        args: func_args![value: "abcdefg", substring: "ABC", case_sensitive: false],
        want: Ok(value!(true)),
    }
}

bench_function! {
    string => vrl::stdlib::String;

    string {
        args: func_args![value: "2"],
        want: Ok("2")
    }
}

bench_function! {
    strip_ansi_escape_codes => vrl::stdlib::StripAnsiEscapeCodes;

    literal {
        args: func_args![value: "\x1b[46mfoo\x1b[0m bar"],
        want: Ok("foo bar")
    }
}

bench_function! {
    strip_whitespace => vrl::stdlib::StripWhitespace;

    literal {
        args: func_args![
            value:" \u{3000}\u{205F}\u{202F}\u{A0}\u{9} ❤❤ hi there ❤❤  \u{9}\u{A0}\u{202F}\u{205F}\u{3000}"
        ],
        want: Ok("❤❤ hi there ❤❤")
    }
}

bench_function! {
    strlen => vrl::stdlib::Strlen;

    literal {
        args: func_args![value: "ñandú"],
        want: Ok(5)
    }
}

bench_function! {
    tag_types_externally => vrl::stdlib::TagTypesExternally;

    tag_bytes {
        args: func_args![value: "foo"],
        want: Ok(btreemap! {
            "string" => "foo",
        }),
    }

    tag_integer {
        args: func_args![value: 123],
        want: Ok(btreemap! {
            "integer" => 123
        }),
    }

    tag_float {
        args: func_args![value: 123.45],
        want: Ok(btreemap! {
            "float" => 123.45
        }),
    }

    tag_boolean {
        args: func_args![value: true],
        want: Ok(btreemap! {
            "boolean" => true
        }),
    }

    tag_map {
        args: func_args![value: btreemap! {"foo" => "bar"}],
        want: Ok(btreemap! {
            "foo" => btreemap! {
                "string" => "bar"
            }
        }),
    }

    tag_array {
        args: func_args![value: vec!["foo"]],
        want: Ok(vec![
            btreemap! {
                "string" => "foo"
            },
        ]),
    }

    tag_timestamp {
        args: func_args![value: Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)],
        want: Ok(btreemap! {
            "timestamp" => Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)
        }),
    }

    tag_regex {
        args: func_args![value: Regex::new(".*").unwrap()],
        want: Ok(btreemap! {
            "regex" => Regex::new(".*").unwrap()
        }),
    }

    tag_null {
        args: func_args![value: Value::Null],
        want: Ok(Value::Null),
    }
}

bench_function! {
    tally => vrl::stdlib::Tally;

    default {
        args: func_args![
            value: value!(["bar", "foo", "baz", "foo"]),
        ],
        want: Ok(value!({"bar": 1, "foo": 2, "baz": 1})),
    }
}

bench_function! {
    tally_value => vrl::stdlib::TallyValue;

    default {
        args: func_args![
            array: value!(["bar", "foo", "baz", "foo"]),
            value: "foo",
        ],
        want: Ok(value!(2)),
    }
}

bench_function! {
    timestamp => vrl::stdlib::Timestamp;

    timestamp {
        args: func_args![value: Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)],
        want: Ok(value!(Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0))),
    }
}

bench_function! {
    to_bool => vrl::stdlib::ToBool;

    string {
        args: func_args![value: "true"],
        want: Ok(true)
    }

    r#bool {
        args: func_args![value: true],
        want: Ok(true)
    }

    int {
        args: func_args![value: 20],
        want: Ok(true)
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok(false)
    }
}

bench_function! {
    to_float => vrl::stdlib::ToFloat;

    string {
        args: func_args![value: "2.0"],
        want: Ok(2.0)
    }

    r#bool {
        args: func_args![value: true],
        want: Ok(1.0)
    }

    float {
        args: func_args![value: 1.0],
        want: Ok(1.0)
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok(0.0)
    }
}

bench_function! {
    to_int => vrl::stdlib::ToInt;

    string {
        args: func_args![value: "2"],
        want: Ok(2)
    }

    r#bool {
        args: func_args![value: true],
        want: Ok(1)
    }

    int {
        args: func_args![value: 1],
        want: Ok(1)
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok(0)
    }
}

bench_function! {
    to_regex => vrl::stdlib::ToRegex;

    regex {
        args: func_args![value: "^foo.*bar.*baz"],
        want: Ok(Regex::new("^foo.*bar.*baz").expect("regex is valid"))
    }
}

bench_function! {
    to_string => vrl::stdlib::ToString;

    string {
        args: func_args![value: "2"],
        want: Ok("2")
    }

    r#bool {
        args: func_args![value: true],
        want: Ok("true")
    }

    int {
        args: func_args![value: 1],
        want: Ok("1")
    }

    null {
        args: func_args![value: value!(null)],
        want: Ok("")
    }
}

bench_function! {
    to_syslog_facility => vrl::stdlib::ToSyslogFacility;

    literal {
        args: func_args![value: value!(23)],
        want: Ok(value!("local7")),
    }
}

bench_function! {
    to_syslog_level => vrl::stdlib::ToSyslogLevel;

    literal {
        args: func_args![value: value!(5)],
        want: Ok(value!("notice")),
    }
}

bench_function! {
    to_syslog_severity => vrl::stdlib::ToSyslogSeverity;

    literal {
        args: func_args![value: value!("info")],
        want: Ok(value!(6)),
    }
}

bench_function! {
    to_unix_timestamp => vrl::stdlib::ToUnixTimestamp;

    default {
        args: func_args![value: Utc.ymd(2021, 1, 1).and_hms_milli(0, 0, 0, 0)],
        want: Ok(1609459200),
    }
}

bench_function! {
    truncate => vrl::stdlib::Truncate;

    ellipsis {
        args: func_args![
            value: "Supercalifragilisticexpialidocious",
            limit: 5,
            suffix: "...",
        ],
        want: Ok("Super..."),
    }

    no_suffix {
        args: func_args![
            value: "Supercalifragilisticexpialidocious",
            limit: 5,
        ],
        want: Ok("Super"),
    }
}

bench_function! {
    unique => vrl::stdlib::Unique;

    default {
        args: func_args![
            value: value!(["bar", "foo", "baz", "foo"]),
        ],
        want: Ok(value!(["bar", "foo", "baz"])),
    }

    mixed_values {
        args: func_args![
            value: value!(["foo", [1,2,3], "123abc", 1, true, [1,2,3], "foo", true, 1]),
        ],
        want: Ok(value!(["foo", [1,2,3], "123abc", 1, true])),
    }
}

bench_function! {
    upcase => vrl::stdlib::Upcase;

    literal {
        args: func_args![value: "foo"],
        want: Ok("FOO")
    }
}

bench_function! {
    values => vrl::stdlib::Values;

    literal {
        args: func_args![value: value!({"key1": "val1", "key2": "val2"})],
        want: Ok(value!(["val1", "val2"])),
    }
}

bench_function! {
    community_id => vrl::stdlib::CommunityID;

    literal {
        args: func_args![source_ip: "1.2.3.4", destination_ip: "5.6.7.8", protocol: 6, source_port: 1122, destination_port: 3344],
        want: Ok("1:wCb3OG7yAFWelaUydu0D+125CLM="),
    }
}
