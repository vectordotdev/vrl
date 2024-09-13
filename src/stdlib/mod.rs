#![deny(
    warnings,
    clippy::all,
    clippy::pedantic,
    unreachable_pub,
    unused_allocation,
    unused_extern_crates,
    unused_assignments,
    unused_comparisons
)]
#![allow(
    deprecated,
    clippy::cast_possible_truncation, // allowed in initial deny commit
    clippy::cast_precision_loss, // allowed in initial deny commit
    clippy::cast_sign_loss, // allowed in initial deny commit
    clippy::default_trait_access, // allowed in initial deny commit
    clippy::doc_markdown, // allowed in initial deny commit
    clippy::inefficient_to_string, // allowed in initial deny commit
    clippy::match_bool, // allowed in initial deny commit
    clippy::match_same_arms, // allowed in initial deny commit
    clippy::needless_pass_by_value, // allowed in initial deny commit
    clippy::semicolon_if_nothing_returned,  // allowed in initial deny commit
    clippy::similar_names, // allowed in initial deny commit
    clippy::single_match_else, // allowed in initial deny commit
    clippy::struct_excessive_bools,  // allowed in initial deny commit
    clippy::too_many_lines, // allowed in initial deny commit
    clippy::trivially_copy_pass_by_ref, // allowed in initial deny commit
)]

pub use wasm_unsupported_function::WasmUnsupportedFunction;

use crate::compiler::Function;

mod string_utils;
mod util;
mod wasm_unsupported_function;

cfg_if::cfg_if! {
    if #[cfg(feature = "stdlib")] {
        mod abs;
        mod append;
        mod array;
        mod assert;
        mod assert_eq;
        mod boolean;
        mod ceil;
        mod casing;
        mod chunks;
        mod compact;
        mod contains;
        mod contains_all;
        mod decode_base16;
        mod decode_base64;
        mod decode_gzip;
        mod decode_mime_q;
        mod decode_percent;
        mod decode_punycode;
        mod decode_snappy;
        mod decode_zlib;
        mod decode_zstd;
        mod decrypt;
        mod del;
        mod dns_lookup;
        mod downcase;
        mod encode_base16;
        mod encode_base64;
        mod encode_gzip;
        mod encode_json;
        mod encode_key_value;
        mod encode_logfmt;
        mod encode_percent;
        mod encode_proto;
        mod encode_punycode;
        mod encode_snappy;
        mod encode_zlib;
        mod encode_zstd;
        mod encrypt;
        mod ends_with;
        mod exists;
        mod filter;
        mod find;
        mod flatten;
        mod float;
        mod floor;
        mod for_each;
        mod format_int;
        mod format_number;
        mod format_timestamp;
        mod from_unix_timestamp;
        mod get;
        mod get_env_var;
        mod get_hostname;
        mod get_timezone_name;
        mod hmac;
        mod includes;
        mod integer;
        mod ip_aton;
        mod ip_cidr_contains;
        mod ip_ntoa;
        mod ip_ntop;
        mod ip_pton;
        mod ip_subnet;
        mod ip_to_ipv6;
        mod ipv6_to_ipv4;
        mod is_array;
        mod is_boolean;
        mod is_empty;
        mod is_float;
        mod is_integer;
        mod is_ipv4;
        mod is_ipv6;
        mod is_json;
        mod is_null;
        mod is_nullish;
        mod is_object;
        mod is_regex;
        mod is_string;
        mod is_timestamp;
        mod join;
        mod keys;
        mod length;
        mod log;
        mod log_util;
        mod map_keys;
        mod map_values;
        mod r#match;
        mod match_any;
        mod match_array;
        mod match_datadog_query;
        mod md5;
        mod merge;
        mod mod_func;
        mod now;
        mod object;
        mod parse_apache_log;
        mod parse_aws_alb_log;
        mod parse_aws_cloudwatch_log_subscription_message;
        mod parse_aws_vpc_flow_log;
        mod parse_cef;
        mod parse_common_log;
        mod parse_csv;
        mod parse_duration;
        mod parse_etld;
        mod parse_float;
        mod parse_glog;
        mod parse_grok;
        mod parse_groks;
        mod parse_influxdb;
        mod parse_int;
        mod parse_json;
        mod parse_key_value;
        mod parse_klog;
        mod parse_linux_authorization;
        mod parse_logfmt;
        mod parse_nginx_log;
        mod parse_proto;
        mod parse_query_string;
        mod parse_regex;
        mod parse_regex_all;
        mod parse_ruby_hash;
        mod parse_syslog;
        mod parse_timestamp;
        mod parse_tokens;
        mod parse_url;
        mod parse_user_agent;
        mod parse_xml;
        mod push;
        mod random_bool;
        mod random_bytes;
        mod random_float;
        mod random_int;
        mod redact;
        mod remove;
        mod replace;
        mod replace_with;
        mod reverse_dns;
        mod round;
        mod seahash;
        mod set;
        mod sha1;
        mod sha2;
        mod sha3;
        mod sieve;
        mod slice;
        mod split;
        mod starts_with;
        mod string;
        mod strip_ansi_escape_codes;
        mod strip_whitespace;
        mod strlen;
        mod tag_types_externally;
        mod tally;
        mod tally_value;
        mod timestamp;
        mod to_bool;
        mod to_float;
        mod to_int;
        mod to_regex;
        mod to_string;
        mod to_syslog_facility;
        mod to_syslog_level;
        mod to_syslog_severity;
        mod to_unix_timestamp;
        mod community_id;
        mod truncate;
        mod unflatten;
        mod type_def;
        mod unique;
        mod unnest;
        mod upcase;
        mod uuid_from_friendly_id;
        mod uuid_v4;
        mod uuid_v7;
        mod values;

        // -----------------------------------------------------------------------------

        pub use self::hmac::Hmac;
        pub use abs::Abs;
        pub use append::Append;
        pub use assert::Assert;
        pub use assert_eq::AssertEq;
        pub use boolean::Boolean;
        pub use ceil::Ceil;
        pub use chunks::Chunks;
        pub use compact::Compact;
        pub use contains::Contains;
        pub use contains_all::ContainsAll;
        pub use decode_base16::DecodeBase16;
        pub use decode_base64::DecodeBase64;
        pub use decode_gzip::DecodeGzip;
        pub use decode_mime_q::DecodeMimeQ;
        pub use decode_percent::DecodePercent;
        pub use decode_punycode::DecodePunycode;
        pub use decode_snappy::DecodeSnappy;
        pub use decode_zlib::DecodeZlib;
        pub use decode_zstd::DecodeZstd;
        pub use decrypt::Decrypt;
        pub use del::Del;
        pub use dns_lookup::DnsLookup;
        pub use downcase::Downcase;
        pub use casing::camelcase::Camelcase;
        pub use casing::pascalcase::Pascalcase;
        pub use casing::snakecase::Snakecase;
        pub use casing::screamingsnakecase::ScreamingSnakecase;
        pub use casing::kebabcase::Kebabcase;
        pub use encode_base16::EncodeBase16;
        pub use encode_base64::EncodeBase64;
        pub use encode_gzip::EncodeGzip;
        pub use encode_json::EncodeJson;
        pub use encode_key_value::EncodeKeyValue;
        pub use encode_logfmt::EncodeLogfmt;
        pub use encode_percent::EncodePercent;
        pub use encode_proto::EncodeProto;
        pub use encode_punycode::EncodePunycode;
        pub use encode_snappy::EncodeSnappy;
        pub use encode_zlib::EncodeZlib;
        pub use encode_zstd::EncodeZstd;
        pub use encrypt::Encrypt;
        pub use ends_with::EndsWith;
        pub use exists::Exists;
        pub use filter::Filter;
        pub use find::Find;
        pub use flatten::Flatten;
        pub use float::Float;
        pub use floor::Floor;
        pub use for_each::ForEach;
        pub use format_int::FormatInt;
        pub use format_number::FormatNumber;
        pub use format_timestamp::FormatTimestamp;
        pub use from_unix_timestamp::FromUnixTimestamp;
        pub use self::community_id::CommunityID;
        pub use get::Get;
        pub use get_env_var::GetEnvVar;
        pub use get_hostname::GetHostname;
        pub use get_timezone_name::GetTimezoneName;
        pub use get_timezone_name::get_name_for_timezone;
        pub use includes::Includes;
        pub use integer::Integer;
        pub use ip_aton::IpAton;
        pub use ip_cidr_contains::IpCidrContains;
        pub use ip_ntoa::IpNtoa;
        pub use ip_ntop::IpNtop;
        pub use ip_pton::IpPton;
        pub use ip_subnet::IpSubnet;
        pub use ip_to_ipv6::IpToIpv6;
        pub use ipv6_to_ipv4::Ipv6ToIpV4;
        pub use is_array::IsArray;
        pub use is_boolean::IsBoolean;
        pub use is_empty::IsEmpty;
        pub use is_float::IsFloat;
        pub use is_integer::IsInteger;
        pub use is_ipv4::IsIpv4;
        pub use is_ipv6::IsIpv6;
        pub use is_json::IsJson;
        pub use is_null::IsNull;
        pub use is_nullish::IsNullish;
        pub use is_object::IsObject;
        pub use is_regex::IsRegex;
        pub use is_string::IsString;
        pub use is_timestamp::IsTimestamp;
        pub use join::Join;
        pub use keys::Keys;
        pub use length::Length;
        pub use log::Log;
        pub use map_keys::MapKeys;
        pub use map_values::MapValues;
        pub use match_any::MatchAny;
        pub use match_array::MatchArray;
        pub use match_datadog_query::MatchDatadogQuery;
        pub use merge::Merge;
        pub use mod_func::Mod;
        pub use now::Now;
        pub use object::Object;
        pub use parse_apache_log::ParseApacheLog;
        pub use parse_aws_alb_log::ParseAwsAlbLog;
        pub use parse_aws_cloudwatch_log_subscription_message::ParseAwsCloudWatchLogSubscriptionMessage;
        pub use parse_aws_vpc_flow_log::ParseAwsVpcFlowLog;
        pub use parse_cef::ParseCef;
        pub use parse_common_log::ParseCommonLog;
        pub use parse_csv::ParseCsv;
        pub use parse_duration::ParseDuration;
        pub use parse_float::ParseFloat;
        pub use parse_etld::ParseEtld;
        pub use parse_glog::ParseGlog;
        pub use parse_grok::ParseGrok;
        pub use parse_groks::ParseGroks;
        pub use parse_influxdb::ParseInfluxDB;
        pub use parse_int::ParseInt;
        pub use parse_json::ParseJson;
        pub use parse_key_value::ParseKeyValue;
        pub use parse_klog::ParseKlog;
        pub use parse_linux_authorization::ParseLinuxAuthorization;
        pub use parse_logfmt::ParseLogFmt;
        pub use parse_nginx_log::ParseNginxLog;
        pub use parse_proto::ParseProto;
        pub use parse_query_string::ParseQueryString;
        pub use parse_regex::ParseRegex;
        pub use parse_regex_all::ParseRegexAll;
        pub use parse_ruby_hash::ParseRubyHash;
        pub use parse_syslog::ParseSyslog;
        pub use parse_timestamp::ParseTimestamp;
        pub use parse_tokens::ParseTokens;
        pub use parse_url::ParseUrl;
        pub use parse_user_agent::ParseUserAgent;
        pub use parse_xml::ParseXml;
        pub use push::Push;
        pub use r#match::Match;
        pub use random_bool::RandomBool;
        pub use random_bytes::RandomBytes;
        pub use random_float::RandomFloat;
        pub use random_int::RandomInt;
        pub use redact::Redact;
        pub use remove::Remove;
        pub use replace::Replace;
        pub use replace_with::ReplaceWith;
        pub use reverse_dns::ReverseDns;
        pub use round::Round;
        pub use set::Set;
        pub use sha2::Sha2;
        pub use sha3::Sha3;
        pub use sieve::Sieve;
        pub use slice::Slice;
        pub use split::Split;
        pub use starts_with::StartsWith;
        pub use string::String;
        pub use strip_ansi_escape_codes::StripAnsiEscapeCodes;
        pub use strip_whitespace::StripWhitespace;
        pub use strlen::Strlen;
        pub use tag_types_externally::TagTypesExternally;
        pub use tally::Tally;
        pub use tally_value::TallyValue;
        pub use timestamp::Timestamp;
        pub use to_bool::ToBool;
        pub use to_float::ToFloat;
        pub use to_int::ToInt;
        pub use to_regex::ToRegex;
        pub use to_string::ToString;
        pub use to_syslog_facility::ToSyslogFacility;
        pub use to_syslog_level::ToSyslogLevel;
        pub use to_syslog_severity::ToSyslogSeverity;
        pub use to_unix_timestamp::ToUnixTimestamp;
        pub use truncate::Truncate;
        pub use type_def::TypeDef;
        pub use unflatten::Unflatten;
        pub use unique::Unique;
        pub use unnest::Unnest;
        pub use upcase::Upcase;
        pub use uuid_from_friendly_id::UuidFromFriendlyId;
        pub use uuid_v4::UuidV4;
        pub use uuid_v7::UuidV7;
        pub use values::Values;
        pub use self::array::Array;
        pub use self::md5::Md5;
        pub use self::seahash::Seahash;
        pub use self::sha1::Sha1;
    }
}

#[cfg(feature = "stdlib")]
#[must_use]
pub fn all() -> Vec<Box<dyn Function>> {
    vec![
        Box::new(Abs),
        Box::new(Append),
        Box::new(Array),
        Box::new(Assert),
        Box::new(AssertEq),
        Box::new(Boolean),
        Box::new(Camelcase),
        Box::new(Ceil),
        Box::new(Chunks),
        Box::new(Compact),
        Box::new(Contains),
        Box::new(ContainsAll),
        Box::new(DecodeBase16),
        Box::new(DecodeBase64),
        Box::new(DecodeGzip),
        Box::new(DecodePercent),
        Box::new(DecodePunycode),
        Box::new(DecodeMimeQ),
        Box::new(DecodeSnappy),
        Box::new(DecodeZlib),
        Box::new(DecodeZstd),
        Box::new(Decrypt),
        Box::new(Del),
        Box::new(DnsLookup),
        Box::new(Downcase),
        Box::new(EncodeBase16),
        Box::new(EncodeBase64),
        Box::new(EncodeGzip),
        Box::new(EncodeJson),
        Box::new(EncodeKeyValue),
        Box::new(EncodeLogfmt),
        Box::new(EncodePercent),
        Box::new(EncodeProto),
        Box::new(EncodePunycode),
        Box::new(EncodeSnappy),
        Box::new(EncodeZlib),
        Box::new(EncodeZstd),
        Box::new(Encrypt),
        Box::new(EndsWith),
        Box::new(Exists),
        Box::new(Filter),
        Box::new(Find),
        Box::new(Flatten),
        Box::new(Float),
        Box::new(Floor),
        Box::new(ForEach),
        Box::new(FormatInt),
        Box::new(FormatNumber),
        Box::new(FormatTimestamp),
        Box::new(FromUnixTimestamp),
        Box::new(Get),
        Box::new(GetEnvVar),
        Box::new(GetHostname),
        Box::new(GetTimezoneName),
        Box::new(Hmac),
        Box::new(Includes),
        Box::new(Integer),
        Box::new(IpAton),
        Box::new(IpCidrContains),
        Box::new(IpNtoa),
        Box::new(IpNtop),
        Box::new(IpPton),
        Box::new(IpSubnet),
        Box::new(IpToIpv6),
        Box::new(Ipv6ToIpV4),
        Box::new(IsArray),
        Box::new(IsBoolean),
        Box::new(IsEmpty),
        Box::new(IsFloat),
        Box::new(IsInteger),
        Box::new(IsIpv4),
        Box::new(IsIpv6),
        Box::new(IsJson),
        Box::new(IsNull),
        Box::new(IsNullish),
        Box::new(IsObject),
        Box::new(IsRegex),
        Box::new(IsString),
        Box::new(IsTimestamp),
        Box::new(Join),
        Box::new(Kebabcase),
        Box::new(Keys),
        Box::new(Length),
        Box::new(Log),
        Box::new(MapKeys),
        Box::new(MapValues),
        Box::new(Match),
        Box::new(MatchAny),
        Box::new(MatchArray),
        Box::new(MatchDatadogQuery),
        Box::new(Md5),
        Box::new(Merge),
        Box::new(Mod),
        Box::new(Now),
        Box::new(Object),
        Box::new(ParseApacheLog),
        Box::new(ParseAwsAlbLog),
        Box::new(ParseAwsCloudWatchLogSubscriptionMessage),
        Box::new(ParseAwsVpcFlowLog),
        Box::new(ParseCef),
        Box::new(ParseCommonLog),
        Box::new(ParseCsv),
        Box::new(ParseDuration),
        Box::new(ParseFloat),
        Box::new(ParseEtld),
        Box::new(ParseGlog),
        Box::new(ParseGrok),
        Box::new(ParseGroks),
        Box::new(ParseInfluxDB),
        Box::new(ParseInt),
        Box::new(ParseJson),
        Box::new(ParseKeyValue),
        Box::new(ParseKlog),
        Box::new(ParseLinuxAuthorization),
        Box::new(ParseLogFmt),
        Box::new(ParseNginxLog),
        Box::new(ParseProto),
        Box::new(ParseQueryString),
        Box::new(ParseRegex),
        Box::new(ParseRegexAll),
        Box::new(ParseRubyHash),
        Box::new(ParseSyslog),
        Box::new(ParseTimestamp),
        Box::new(ParseTokens),
        Box::new(ParseUrl),
        Box::new(ParseUserAgent),
        Box::new(ParseXml),
        Box::new(Pascalcase),
        Box::new(Push),
        Box::new(RandomBool),
        Box::new(RandomBytes),
        Box::new(RandomFloat),
        Box::new(RandomInt),
        Box::new(Redact),
        Box::new(Remove),
        Box::new(Replace),
        Box::new(ReplaceWith),
        Box::new(ReverseDns),
        Box::new(Round),
        Box::new(Seahash),
        Box::new(Set),
        Box::new(Sha1),
        Box::new(Sha2),
        Box::new(Sha3),
        Box::new(Sieve),
        Box::new(ScreamingSnakecase),
        Box::new(Snakecase),
        Box::new(Slice),
        Box::new(Split),
        Box::new(StartsWith),
        Box::new(String),
        Box::new(StripAnsiEscapeCodes),
        Box::new(StripWhitespace),
        Box::new(Strlen),
        Box::new(Tally),
        Box::new(TallyValue),
        Box::new(TagTypesExternally),
        Box::new(Timestamp),
        Box::new(ToBool),
        Box::new(ToFloat),
        Box::new(ToInt),
        Box::new(ToRegex),
        Box::new(ToString),
        Box::new(ToSyslogFacility),
        Box::new(ToSyslogLevel),
        Box::new(ToSyslogSeverity),
        Box::new(ToUnixTimestamp),
        Box::new(CommunityID),
        Box::new(Truncate),
        Box::new(TypeDef),
        Box::new(Unflatten),
        Box::new(Unique),
        Box::new(Unnest),
        Box::new(Upcase),
        Box::new(UuidFromFriendlyId),
        Box::new(UuidV4),
        Box::new(UuidV7),
        Box::new(Values),
    ]
}
