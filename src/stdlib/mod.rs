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

mod util;
mod wasm_unsupported_function;
use crate::compiler::Function;
pub use wasm_unsupported_function::WasmUnsupportedFunction;

#[cfg(feature = "stdlib_abs")]
mod abs;
#[cfg(feature = "stdlib_append")]
mod append;
#[cfg(feature = "stdlib_array")]
mod array;
#[cfg(feature = "stdlib_assert")]
mod assert;
#[cfg(feature = "stdlib_assert_eq")]
mod assert_eq;
#[cfg(feature = "stdlib_boolean")]
mod boolean;
#[cfg(feature = "stdlib_ceil")]
mod ceil;
#[cfg(feature = "stdlib_chunks")]
mod chunks;
#[cfg(feature = "stdlib_compact")]
mod compact;
#[cfg(feature = "stdlib_contains")]
mod contains;
#[cfg(feature = "stdlib_decode_base16")]
mod decode_base16;
#[cfg(feature = "stdlib_decode_base64")]
mod decode_base64;
#[cfg(feature = "stdlib_decode_gzip")]
mod decode_gzip;
#[cfg(feature = "stdlib_decode_mime_q")]
mod decode_mime_q;
#[cfg(feature = "stdlib_decode_percent")]
mod decode_percent;
#[cfg(feature = "stdlib_decode_zlib")]
mod decode_zlib;
#[cfg(feature = "stdlib_decode_zstd")]
mod decode_zstd;
#[cfg(feature = "stdlib_decrypt")]
mod decrypt;
#[cfg(feature = "stdlib_del")]
mod del;
#[cfg(feature = "stdlib_downcase")]
mod downcase;
#[cfg(feature = "stdlib_encode_base16")]
mod encode_base16;
#[cfg(feature = "stdlib_encode_base64")]
mod encode_base64;
#[cfg(feature = "stdlib_encode_gzip")]
mod encode_gzip;
#[cfg(feature = "stdlib_encode_json")]
mod encode_json;
#[cfg(feature = "stdlib_encode_key_value")]
mod encode_key_value;
#[cfg(feature = "stdlib_encode_logfmt")]
mod encode_logfmt;
#[cfg(feature = "stdlib_encode_percent")]
mod encode_percent;
#[cfg(feature = "stdlib_encode_zlib")]
mod encode_zlib;
#[cfg(feature = "stdlib_encode_zstd")]
mod encode_zstd;
#[cfg(feature = "stdlib_encrypt")]
mod encrypt;
#[cfg(feature = "stdlib_ends_with")]
mod ends_with;
#[cfg(feature = "stdlib_exists")]
mod exists;
#[cfg(feature = "stdlib_filter")]
mod filter;
#[cfg(feature = "stdlib_find")]
mod find;
#[cfg(feature = "stdlib_flatten")]
mod flatten;
#[cfg(feature = "stdlib_float")]
mod float;
#[cfg(feature = "stdlib_floor")]
mod floor;
#[cfg(feature = "stdlib_for_each")]
mod for_each;
#[cfg(feature = "stdlib_format_int")]
mod format_int;
#[cfg(feature = "stdlib_format_number")]
mod format_number;
#[cfg(feature = "stdlib_format_timestamp")]
mod format_timestamp;
#[cfg(feature = "stdlib_get")]
mod get;
#[cfg(feature = "stdlib_get_env_var")]
mod get_env_var;
#[cfg(feature = "stdlib_get_hostname")]
mod get_hostname;
#[cfg(feature = "stdlib_hmac")]
mod hmac;
#[cfg(feature = "stdlib_includes")]
mod includes;
#[cfg(feature = "stdlib_integer")]
mod integer;
#[cfg(feature = "stdlib_ip_aton")]
mod ip_aton;
#[cfg(feature = "stdlib_ip_cidr_contains")]
mod ip_cidr_contains;
#[cfg(feature = "stdlib_ip_ntoa")]
mod ip_ntoa;
#[cfg(feature = "stdlib_ip_ntop")]
mod ip_ntop;
#[cfg(feature = "stdlib_ip_pton")]
mod ip_pton;
#[cfg(feature = "stdlib_ip_subnet")]
mod ip_subnet;
#[cfg(feature = "stdlib_ip_to_ipv6")]
mod ip_to_ipv6;
#[cfg(feature = "stdlib_ipv6_to_ipv4")]
mod ipv6_to_ipv4;
#[cfg(feature = "stdlib_is_array")]
mod is_array;
#[cfg(feature = "stdlib_is_boolean")]
mod is_boolean;
#[cfg(feature = "stdlib_is_empty")]
mod is_empty;
#[cfg(feature = "stdlib_is_float")]
mod is_float;
#[cfg(feature = "stdlib_is_integer")]
mod is_integer;
#[cfg(feature = "stdlib_is_ipv4")]
mod is_ipv4;
#[cfg(feature = "stdlib_is_ipv6")]
mod is_ipv6;
#[cfg(feature = "stdlib_is_json")]
mod is_json;
#[cfg(feature = "stdlib_is_null")]
mod is_null;
#[cfg(feature = "stdlib_is_nullish")]
mod is_nullish;
#[cfg(feature = "stdlib_is_object")]
mod is_object;
#[cfg(feature = "stdlib_is_regex")]
mod is_regex;
#[cfg(feature = "stdlib_is_string")]
mod is_string;
#[cfg(feature = "stdlib_is_timestamp")]
mod is_timestamp;
#[cfg(feature = "stdlib_join")]
mod join;
#[cfg(feature = "stdlib_keys")]
mod keys;
#[cfg(feature = "stdlib_length")]
mod length;
#[cfg(feature = "stdlib_log")]
mod log;
#[cfg(any(
    feature = "stdlib_parse_common_log",
    feature = "stdlib_parse_apache_log",
    feature = "stdlib_parse_nginx_log"
))]
mod log_util;
#[cfg(feature = "stdlib_map_keys")]
mod map_keys;
#[cfg(feature = "stdlib_map_values")]
mod map_values;
#[cfg(feature = "stdlib_match")]
mod r#match;
#[cfg(feature = "stdlib_match_any")]
mod match_any;
#[cfg(feature = "stdlib_match_array")]
mod match_array;
#[cfg(feature = "stdlib_match_datadog_query")]
mod match_datadog_query;
#[cfg(feature = "stdlib_md5")]
mod md5;
#[cfg(feature = "stdlib_merge")]
mod merge;
#[cfg(feature = "stdlib_mod")]
mod mod_func;
#[cfg(feature = "stdlib_now")]
mod now;
#[cfg(feature = "stdlib_object")]
mod object;
#[cfg(feature = "stdlib_only_fields")]
mod only_fields;
#[cfg(feature = "stdlib_parse_apache_log")]
mod parse_apache_log;
#[cfg(feature = "stdlib_parse_aws_alb_log")]
mod parse_aws_alb_log;
#[cfg(feature = "stdlib_parse_aws_cloudwatch_log_subscription_message")]
mod parse_aws_cloudwatch_log_subscription_message;
#[cfg(feature = "stdlib_parse_aws_vpc_flow_log")]
mod parse_aws_vpc_flow_log;
#[cfg(feature = "stdlib_parse_cef")]
mod parse_cef;
#[cfg(feature = "stdlib_parse_common_log")]
mod parse_common_log;
#[cfg(feature = "stdlib_parse_csv")]
mod parse_csv;
#[cfg(feature = "stdlib_parse_duration")]
mod parse_duration;
#[cfg(feature = "stdlib_parse_glog")]
mod parse_glog;
#[cfg(feature = "stdlib_parse_grok")]
mod parse_grok;
#[cfg(feature = "stdlib_parse_groks")]
mod parse_groks;
#[cfg(feature = "stdlib_parse_int")]
mod parse_int;
#[cfg(feature = "stdlib_parse_json")]
mod parse_json;
#[cfg(feature = "stdlib_parse_key_value")]
mod parse_key_value;
#[cfg(feature = "stdlib_parse_klog")]
mod parse_klog;
#[cfg(feature = "stdlib_parse_linux_authorization")]
mod parse_linux_authorization;
#[cfg(feature = "stdlib_parse_logfmt")]
mod parse_logfmt;
#[cfg(feature = "stdlib_parse_nginx_log")]
mod parse_nginx_log;
#[cfg(feature = "stdlib_parse_query_string")]
mod parse_query_string;
#[cfg(feature = "stdlib_parse_regex")]
mod parse_regex;
#[cfg(feature = "stdlib_parse_regex_all")]
mod parse_regex_all;
#[cfg(feature = "stdlib_parse_ruby_hash")]
mod parse_ruby_hash;
#[cfg(feature = "stdlib_parse_syslog")]
mod parse_syslog;
#[cfg(feature = "stdlib_parse_timestamp")]
mod parse_timestamp;
#[cfg(feature = "stdlib_parse_tokens")]
mod parse_tokens;
#[cfg(feature = "stdlib_parse_url")]
mod parse_url;
#[cfg(feature = "stdlib_parse_user_agent")]
mod parse_user_agent;
#[cfg(feature = "stdlib_parse_xml")]
mod parse_xml;
#[cfg(feature = "stdlib_push")]
mod push;
#[cfg(feature = "stdlib_random_bool")]
mod random_bool;
#[cfg(feature = "stdlib_random_bytes")]
mod random_bytes;
#[cfg(feature = "stdlib_random_float")]
mod random_float;
#[cfg(feature = "stdlib_random_int")]
mod random_int;
#[cfg(feature = "stdlib_redact")]
mod redact;
#[cfg(feature = "stdlib_remove")]
mod remove;
#[cfg(feature = "stdlib_replace")]
mod replace;
#[cfg(feature = "stdlib_reverse_dns")]
mod reverse_dns;
#[cfg(feature = "stdlib_round")]
mod round;
#[cfg(feature = "stdlib_seahash")]
mod seahash;
#[cfg(feature = "stdlib_set")]
mod set;
#[cfg(feature = "stdlib_sha1")]
mod sha1;
#[cfg(feature = "stdlib_sha2")]
mod sha2;
#[cfg(feature = "stdlib_sha3")]
mod sha3;
#[cfg(feature = "stdlib_slice")]
mod slice;
#[cfg(feature = "stdlib_split")]
mod split;
#[cfg(feature = "stdlib_starts_with")]
mod starts_with;
#[cfg(feature = "stdlib_string")]
mod string;
#[cfg(feature = "stdlib_strip_ansi_escape_codes")]
mod strip_ansi_escape_codes;
#[cfg(feature = "stdlib_strip_whitespace")]
mod strip_whitespace;
#[cfg(feature = "stdlib_strlen")]
mod strlen;
#[cfg(feature = "stdlib_tag_types_externally")]
mod tag_types_externally;
#[cfg(feature = "stdlib_tally")]
mod tally;
#[cfg(feature = "stdlib_tally_value")]
mod tally_value;
#[cfg(feature = "stdlib_timestamp")]
mod timestamp;
#[cfg(feature = "stdlib_to_bool")]
mod to_bool;
#[cfg(feature = "stdlib_to_float")]
mod to_float;
#[cfg(feature = "stdlib_to_int")]
mod to_int;
#[cfg(feature = "stdlib_to_regex")]
mod to_regex;
#[cfg(feature = "stdlib_to_string")]
mod to_string;
#[cfg(feature = "stdlib_to_syslog_facility")]
mod to_syslog_facility;
#[cfg(feature = "stdlib_to_syslog_level")]
mod to_syslog_level;
#[cfg(feature = "stdlib_to_syslog_severity")]
mod to_syslog_severity;
#[cfg(feature = "stdlib_to_timestamp")]
mod to_timestamp;
#[cfg(feature = "stdlib_to_unix_timestamp")]
mod to_unix_timestamp;
#[cfg(feature = "stdlib_truncate")]
mod truncate;
#[cfg(feature = "stdlib_type_def")]
mod type_def;
#[cfg(feature = "stdlib_unique")]
mod unique;
#[cfg(feature = "stdlib_unnest")]
mod unnest;
#[cfg(feature = "stdlib_upcase")]
mod upcase;
#[cfg(feature = "stdlib_uuid_v4")]
mod uuid_v4;
#[cfg(feature = "stdlib_values")]
mod values;

// -----------------------------------------------------------------------------

#[cfg(feature = "stdlib_hmac")]
pub use self::hmac::Hmac;
#[cfg(feature = "stdlib_abs")]
pub use abs::Abs;
#[cfg(feature = "stdlib_append")]
pub use append::Append;
#[cfg(feature = "stdlib_assert")]
pub use assert::Assert;
#[cfg(feature = "stdlib_assert_eq")]
pub use assert_eq::AssertEq;
#[cfg(feature = "stdlib_boolean")]
pub use boolean::Boolean;
#[cfg(feature = "stdlib_ceil")]
pub use ceil::Ceil;
#[cfg(feature = "stdlib_chunks")]
pub use chunks::Chunks;
#[cfg(feature = "stdlib_compact")]
pub use compact::Compact;
#[cfg(feature = "stdlib_contains")]
pub use contains::Contains;
#[cfg(feature = "stdlib_decode_base16")]
pub use decode_base16::DecodeBase16;
#[cfg(feature = "stdlib_decode_base64")]
pub use decode_base64::DecodeBase64;
#[cfg(feature = "stdlib_decode_gzip")]
pub use decode_gzip::DecodeGzip;
#[cfg(feature = "stdlib_decode_mime_q")]
pub use decode_mime_q::DecodeMimeQ;
#[cfg(feature = "stdlib_decode_percent")]
pub use decode_percent::DecodePercent;
#[cfg(feature = "stdlib_decode_zlib")]
pub use decode_zlib::DecodeZlib;
#[cfg(feature = "stdlib_decode_zstd")]
pub use decode_zstd::DecodeZstd;
#[cfg(feature = "stdlib_decrypt")]
pub use decrypt::Decrypt;
#[cfg(feature = "stdlib_del")]
pub use del::Del;
#[cfg(feature = "stdlib_downcase")]
pub use downcase::Downcase;
#[cfg(feature = "stdlib_encode_base16")]
pub use encode_base16::EncodeBase16;
#[cfg(feature = "stdlib_encode_base64")]
pub use encode_base64::EncodeBase64;
#[cfg(feature = "stdlib_encode_gzip")]
pub use encode_gzip::EncodeGzip;
#[cfg(feature = "stdlib_encode_json")]
pub use encode_json::EncodeJson;
#[cfg(feature = "stdlib_encode_key_value")]
pub use encode_key_value::EncodeKeyValue;
#[cfg(feature = "stdlib_encode_logfmt")]
pub use encode_logfmt::EncodeLogfmt;
#[cfg(feature = "stdlib_encode_percent")]
pub use encode_percent::EncodePercent;
#[cfg(feature = "stdlib_encode_zlib")]
pub use encode_zlib::EncodeZlib;
#[cfg(feature = "stdlib_encode_zstd")]
pub use encode_zstd::EncodeZstd;
#[cfg(feature = "stdlib_encrypt")]
pub use encrypt::Encrypt;
#[cfg(feature = "stdlib_ends_with")]
pub use ends_with::EndsWith;
#[cfg(feature = "stdlib_exists")]
pub use exists::Exists;
#[cfg(feature = "stdlib_filter")]
pub use filter::Filter;
#[cfg(feature = "stdlib_find")]
pub use find::Find;
#[cfg(feature = "stdlib_flatten")]
pub use flatten::Flatten;
#[cfg(feature = "stdlib_float")]
pub use float::Float;
#[cfg(feature = "stdlib_floor")]
pub use floor::Floor;
#[cfg(feature = "stdlib_for_each")]
pub use for_each::ForEach;
#[cfg(feature = "stdlib_format_int")]
pub use format_int::FormatInt;
#[cfg(feature = "stdlib_format_number")]
pub use format_number::FormatNumber;
#[cfg(feature = "stdlib_format_timestamp")]
pub use format_timestamp::FormatTimestamp;
#[cfg(feature = "stdlib_get")]
pub use get::Get;
#[cfg(feature = "stdlib_get_env_var")]
pub use get_env_var::GetEnvVar;
#[cfg(feature = "stdlib_get_hostname")]
pub use get_hostname::GetHostname;
#[cfg(feature = "stdlib_includes")]
pub use includes::Includes;
#[cfg(feature = "stdlib_integer")]
pub use integer::Integer;
#[cfg(feature = "stdlib_ip_aton")]
pub use ip_aton::IpAton;
#[cfg(feature = "stdlib_ip_cidr_contains")]
pub use ip_cidr_contains::IpCidrContains;
#[cfg(feature = "stdlib_ip_ntoa")]
pub use ip_ntoa::IpNtoa;
#[cfg(feature = "stdlib_ip_ntop")]
pub use ip_ntop::IpNtop;
#[cfg(feature = "stdlib_ip_pton")]
pub use ip_pton::IpPton;
#[cfg(feature = "stdlib_ip_subnet")]
pub use ip_subnet::IpSubnet;
#[cfg(feature = "stdlib_ip_to_ipv6")]
pub use ip_to_ipv6::IpToIpv6;
#[cfg(feature = "stdlib_ipv6_to_ipv4")]
pub use ipv6_to_ipv4::Ipv6ToIpV4;
#[cfg(feature = "stdlib_is_array")]
pub use is_array::IsArray;
#[cfg(feature = "stdlib_is_boolean")]
pub use is_boolean::IsBoolean;
#[cfg(feature = "stdlib_is_empty")]
pub use is_empty::IsEmpty;
#[cfg(feature = "stdlib_is_float")]
pub use is_float::IsFloat;
#[cfg(feature = "stdlib_is_integer")]
pub use is_integer::IsInteger;
#[cfg(feature = "stdlib_is_ipv4")]
pub use is_ipv4::IsIpv4;
#[cfg(feature = "stdlib_is_ipv6")]
pub use is_ipv6::IsIpv6;
#[cfg(feature = "stdlib_is_json")]
pub use is_json::IsJson;
#[cfg(feature = "stdlib_is_null")]
pub use is_null::IsNull;
#[cfg(feature = "stdlib_is_nullish")]
pub use is_nullish::IsNullish;
#[cfg(feature = "stdlib_is_object")]
pub use is_object::IsObject;
#[cfg(feature = "stdlib_is_regex")]
pub use is_regex::IsRegex;
#[cfg(feature = "stdlib_is_string")]
pub use is_string::IsString;
#[cfg(feature = "stdlib_is_timestamp")]
pub use is_timestamp::IsTimestamp;
#[cfg(feature = "stdlib_join")]
pub use join::Join;
#[cfg(feature = "stdlib_keys")]
pub use keys::Keys;
#[cfg(feature = "stdlib_length")]
pub use length::Length;
#[cfg(feature = "stdlib_log")]
pub use log::Log;
#[cfg(feature = "stdlib_map_keys")]
pub use map_keys::MapKeys;
#[cfg(feature = "stdlib_map_values")]
pub use map_values::MapValues;
#[cfg(feature = "stdlib_match_any")]
pub use match_any::MatchAny;
#[cfg(feature = "stdlib_match_array")]
pub use match_array::MatchArray;
#[cfg(feature = "stdlib_match_datadog_query")]
pub use match_datadog_query::MatchDatadogQuery;
#[cfg(feature = "stdlib_merge")]
pub use merge::Merge;
#[cfg(feature = "stdlib_mod")]
pub use mod_func::Mod;
#[cfg(feature = "stdlib_now")]
pub use now::Now;
#[cfg(feature = "stdlib_object")]
pub use object::Object;
#[cfg(feature = "stdlib_only_fields")]
pub use only_fields::OnlyFields;
#[cfg(feature = "stdlib_parse_apache_log")]
pub use parse_apache_log::ParseApacheLog;
#[cfg(feature = "stdlib_parse_aws_alb_log")]
pub use parse_aws_alb_log::ParseAwsAlbLog;
#[cfg(feature = "stdlib_parse_aws_cloudwatch_log_subscription_message")]
pub use parse_aws_cloudwatch_log_subscription_message::ParseAwsCloudWatchLogSubscriptionMessage;
#[cfg(feature = "stdlib_parse_aws_vpc_flow_log")]
pub use parse_aws_vpc_flow_log::ParseAwsVpcFlowLog;
#[cfg(feature = "stdlib_parse_cef")]
pub use parse_cef::ParseCef;
#[cfg(feature = "stdlib_parse_common_log")]
pub use parse_common_log::ParseCommonLog;
#[cfg(feature = "stdlib_parse_csv")]
pub use parse_csv::ParseCsv;
#[cfg(feature = "stdlib_parse_duration")]
pub use parse_duration::ParseDuration;
#[cfg(feature = "stdlib_parse_glog")]
pub use parse_glog::ParseGlog;
#[cfg(feature = "stdlib_parse_grok")]
pub use parse_grok::ParseGrok;
#[cfg(feature = "stdlib_parse_groks")]
pub use parse_groks::ParseGroks;
#[cfg(feature = "stdlib_parse_int")]
pub use parse_int::ParseInt;
#[cfg(feature = "stdlib_parse_json")]
pub use parse_json::ParseJson;
#[cfg(feature = "stdlib_parse_key_value")]
pub use parse_key_value::ParseKeyValue;
#[cfg(feature = "stdlib_parse_klog")]
pub use parse_klog::ParseKlog;
#[cfg(feature = "stdlib_parse_linux_authorization")]
pub use parse_linux_authorization::ParseLinuxAuthorization;
#[cfg(feature = "stdlib_parse_logfmt")]
pub use parse_logfmt::ParseLogFmt;
#[cfg(feature = "stdlib_parse_nginx_log")]
pub use parse_nginx_log::ParseNginxLog;
#[cfg(feature = "stdlib_parse_query_string")]
pub use parse_query_string::ParseQueryString;
#[cfg(feature = "stdlib_parse_regex")]
pub use parse_regex::ParseRegex;
#[cfg(feature = "stdlib_parse_regex_all")]
pub use parse_regex_all::ParseRegexAll;
#[cfg(feature = "stdlib_parse_ruby_hash")]
pub use parse_ruby_hash::ParseRubyHash;
#[cfg(feature = "stdlib_parse_syslog")]
pub use parse_syslog::ParseSyslog;
#[cfg(feature = "stdlib_parse_timestamp")]
pub use parse_timestamp::ParseTimestamp;
#[cfg(feature = "stdlib_parse_tokens")]
pub use parse_tokens::ParseTokens;
#[cfg(feature = "stdlib_parse_url")]
pub use parse_url::ParseUrl;
#[cfg(feature = "stdlib_parse_user_agent")]
pub use parse_user_agent::ParseUserAgent;
#[cfg(feature = "stdlib_parse_xml")]
pub use parse_xml::ParseXml;
#[cfg(feature = "stdlib_push")]
pub use push::Push;
#[cfg(feature = "stdlib_match")]
pub use r#match::Match;
#[cfg(feature = "stdlib_random_bool")]
pub use random_bool::RandomBool;
#[cfg(feature = "stdlib_random_bytes")]
pub use random_bytes::RandomBytes;
#[cfg(feature = "stdlib_random_float")]
pub use random_float::RandomFloat;
#[cfg(feature = "stdlib_random_int")]
pub use random_int::RandomInt;
#[cfg(feature = "stdlib_redact")]
pub use redact::Redact;
#[cfg(feature = "stdlib_remove")]
pub use remove::Remove;
#[cfg(feature = "stdlib_replace")]
pub use replace::Replace;
#[cfg(feature = "stdlib_reverse_dns")]
pub use reverse_dns::ReverseDns;
#[cfg(feature = "stdlib_round")]
pub use round::Round;
#[cfg(feature = "stdlib_set")]
pub use set::Set;
#[cfg(feature = "stdlib_sha2")]
pub use sha2::Sha2;
#[cfg(feature = "stdlib_sha3")]
pub use sha3::Sha3;
#[cfg(feature = "stdlib_slice")]
pub use slice::Slice;
#[cfg(feature = "stdlib_split")]
pub use split::Split;
#[cfg(feature = "stdlib_starts_with")]
pub use starts_with::StartsWith;
#[cfg(feature = "stdlib_string")]
pub use string::String;
#[cfg(feature = "stdlib_strip_ansi_escape_codes")]
pub use strip_ansi_escape_codes::StripAnsiEscapeCodes;
#[cfg(feature = "stdlib_strip_whitespace")]
pub use strip_whitespace::StripWhitespace;
#[cfg(feature = "stdlib_strlen")]
pub use strlen::Strlen;
#[cfg(feature = "stdlib_tag_types_externally")]
pub use tag_types_externally::TagTypesExternally;
#[cfg(feature = "stdlib_tally")]
pub use tally::Tally;
#[cfg(feature = "stdlib_tally_value")]
pub use tally_value::TallyValue;
#[cfg(feature = "stdlib_timestamp")]
pub use timestamp::Timestamp;
#[cfg(feature = "stdlib_to_bool")]
pub use to_bool::ToBool;
#[cfg(feature = "stdlib_to_float")]
pub use to_float::ToFloat;
#[cfg(feature = "stdlib_to_int")]
pub use to_int::ToInt;
#[cfg(feature = "stdlib_to_regex")]
pub use to_regex::ToRegex;
#[cfg(feature = "stdlib_to_string")]
pub use to_string::ToString;
#[cfg(feature = "stdlib_to_syslog_facility")]
pub use to_syslog_facility::ToSyslogFacility;
#[cfg(feature = "stdlib_to_syslog_level")]
pub use to_syslog_level::ToSyslogLevel;
#[cfg(feature = "stdlib_to_syslog_severity")]
pub use to_syslog_severity::ToSyslogSeverity;
#[cfg(feature = "stdlib_to_timestamp")]
pub use to_timestamp::ToTimestamp;
#[cfg(feature = "stdlib_to_unix_timestamp")]
pub use to_unix_timestamp::ToUnixTimestamp;
#[cfg(feature = "stdlib_truncate")]
pub use truncate::Truncate;
#[cfg(feature = "stdlib_type_def")]
pub use type_def::TypeDef;
#[cfg(feature = "stdlib_unique")]
pub use unique::Unique;
#[cfg(feature = "stdlib_unnest")]
pub use unnest::Unnest;
#[cfg(feature = "stdlib_upcase")]
pub use upcase::Upcase;
#[cfg(feature = "stdlib_uuid_v4")]
pub use uuid_v4::UuidV4;
#[cfg(feature = "stdlib_values")]
pub use values::Values;

#[cfg(feature = "stdlib_array")]
pub use self::array::Array;
#[cfg(feature = "stdlib_md5")]
pub use self::md5::Md5;
#[cfg(feature = "stdlib_seahash")]
pub use self::seahash::Seahash;
#[cfg(feature = "stdlib_sha1")]
pub use self::sha1::Sha1;

#[must_use]
pub fn all() -> Vec<Box<dyn Function>> {
    vec![
        #[cfg(feature = "stdlib_abs")]
        Box::new(Abs),
        #[cfg(feature = "stdlib_append")]
        Box::new(Append),
        #[cfg(feature = "stdlib_array")]
        Box::new(Array),
        #[cfg(feature = "stdlib_assert")]
        Box::new(Assert),
        #[cfg(feature = "stdlib_assert_eq")]
        Box::new(AssertEq),
        #[cfg(feature = "stdlib_boolean")]
        Box::new(Boolean),
        #[cfg(feature = "stdlib_ceil")]
        Box::new(Ceil),
        #[cfg(feature = "stdlib_chunks")]
        Box::new(Chunks),
        #[cfg(feature = "stdlib_compact")]
        Box::new(Compact),
        #[cfg(feature = "stdlib_contains")]
        Box::new(Contains),
        #[cfg(feature = "stdlib_decode_base16")]
        Box::new(DecodeBase16),
        #[cfg(feature = "stdlib_decode_base64")]
        Box::new(DecodeBase64),
        #[cfg(feature = "stdlib_decode_gzip")]
        Box::new(DecodeGzip),
        #[cfg(feature = "stdlib_decode_percent")]
        Box::new(DecodePercent),
        #[cfg(feature = "stdlib_decode_mime_q")]
        Box::new(DecodeMimeQ),
        #[cfg(feature = "stdlib_decode_zlib")]
        Box::new(DecodeZlib),
        #[cfg(feature = "stdlib_decode_zstd")]
        Box::new(DecodeZstd),
        #[cfg(feature = "stdlib_decrypt")]
        Box::new(Decrypt),
        #[cfg(feature = "stdlib_del")]
        Box::new(Del),
        #[cfg(feature = "stdlib_downcase")]
        Box::new(Downcase),
        #[cfg(feature = "stdlib_encode_base16")]
        Box::new(EncodeBase16),
        #[cfg(feature = "stdlib_encode_base64")]
        Box::new(EncodeBase64),
        #[cfg(feature = "stdlib_encode_gzip")]
        Box::new(EncodeGzip),
        #[cfg(feature = "stdlib_encode_json")]
        Box::new(EncodeJson),
        #[cfg(feature = "stdlib_encode_key_value")]
        Box::new(EncodeKeyValue),
        #[cfg(feature = "stdlib_encode_logfmt")]
        Box::new(EncodeLogfmt),
        #[cfg(feature = "stdlib_encode_percent")]
        Box::new(EncodePercent),
        #[cfg(feature = "stdlib_encode_zlib")]
        Box::new(EncodeZlib),
        #[cfg(feature = "stdlib_encode_zstd")]
        Box::new(EncodeZstd),
        #[cfg(feature = "stdlib_encrypt")]
        Box::new(Encrypt),
        #[cfg(feature = "stdlib_ends_with")]
        Box::new(EndsWith),
        #[cfg(feature = "stdlib_exists")]
        Box::new(Exists),
        #[cfg(feature = "stdlib_filter")]
        Box::new(Filter),
        #[cfg(feature = "stdlib_find")]
        Box::new(Find),
        #[cfg(feature = "stdlib_flatten")]
        Box::new(Flatten),
        #[cfg(feature = "stdlib_float")]
        Box::new(Float),
        #[cfg(feature = "stdlib_floor")]
        Box::new(Floor),
        #[cfg(feature = "stdlib_for_each")]
        Box::new(ForEach),
        #[cfg(feature = "stdlib_format_int")]
        Box::new(FormatInt),
        #[cfg(feature = "stdlib_format_number")]
        Box::new(FormatNumber),
        #[cfg(feature = "stdlib_format_timestamp")]
        Box::new(FormatTimestamp),
        #[cfg(feature = "stdlib_get")]
        Box::new(Get),
        #[cfg(feature = "stdlib_get_env_var")]
        Box::new(GetEnvVar),
        #[cfg(feature = "stdlib_get_hostname")]
        Box::new(GetHostname),
        #[cfg(feature = "stdlib_hmac")]
        Box::new(Hmac),
        #[cfg(feature = "stdlib_includes")]
        Box::new(Includes),
        #[cfg(feature = "stdlib_integer")]
        Box::new(Integer),
        #[cfg(feature = "stdlib_ip_aton")]
        Box::new(IpAton),
        #[cfg(feature = "stdlib_ip_cidr_contains")]
        Box::new(IpCidrContains),
        #[cfg(feature = "stdlib_ip_ntoa")]
        Box::new(IpNtoa),
        #[cfg(feature = "stdlib_ip_ntop")]
        Box::new(IpNtop),
        #[cfg(feature = "stdlib_ip_pton")]
        Box::new(IpPton),
        #[cfg(feature = "stdlib_ip_subnet")]
        Box::new(IpSubnet),
        #[cfg(feature = "stdlib_ip_to_ipv6")]
        Box::new(IpToIpv6),
        #[cfg(feature = "stdlib_ipv6_to_ipv4")]
        Box::new(Ipv6ToIpV4),
        #[cfg(feature = "stdlib_is_array")]
        Box::new(IsArray),
        #[cfg(feature = "stdlib_is_boolean")]
        Box::new(IsBoolean),
        #[cfg(feature = "stdlib_is_empty")]
        Box::new(IsEmpty),
        #[cfg(feature = "stdlib_is_float")]
        Box::new(IsFloat),
        #[cfg(feature = "stdlib_is_integer")]
        Box::new(IsInteger),
        #[cfg(feature = "stdlib_is_ipv4")]
        Box::new(IsIpv4),
        #[cfg(feature = "stdlib_is_ipv6")]
        Box::new(IsIpv6),
        #[cfg(feature = "stdlib_is_json")]
        Box::new(IsJson),
        #[cfg(feature = "stdlib_is_null")]
        Box::new(IsNull),
        #[cfg(feature = "stdlib_is_nullish")]
        Box::new(IsNullish),
        #[cfg(feature = "stdlib_is_object")]
        Box::new(IsObject),
        #[cfg(feature = "stdlib_is_regex")]
        Box::new(IsRegex),
        #[cfg(feature = "stdlib_is_string")]
        Box::new(IsString),
        #[cfg(feature = "stdlib_is_timestamp")]
        Box::new(IsTimestamp),
        #[cfg(feature = "stdlib_join")]
        Box::new(Join),
        #[cfg(feature = "stdlib_keys")]
        Box::new(Keys),
        #[cfg(feature = "stdlib_length")]
        Box::new(Length),
        #[cfg(feature = "stdlib_log")]
        Box::new(Log),
        #[cfg(feature = "stdlib_map_keys")]
        Box::new(MapKeys),
        #[cfg(feature = "stdlib_map_values")]
        Box::new(MapValues),
        #[cfg(feature = "stdlib_match")]
        Box::new(Match),
        #[cfg(feature = "stdlib_match_any")]
        Box::new(MatchAny),
        #[cfg(feature = "stdlib_match_array")]
        Box::new(MatchArray),
        #[cfg(feature = "stdlib_match_datadog_query")]
        Box::new(MatchDatadogQuery),
        #[cfg(feature = "stdlib_md5")]
        Box::new(Md5),
        #[cfg(feature = "stdlib_merge")]
        Box::new(Merge),
        #[cfg(feature = "stdlib_mod")]
        Box::new(Mod),
        #[cfg(feature = "stdlib_now")]
        Box::new(Now),
        // We are not sure if this is the way we want to expose this functionality yet
        // https://github.com/vectordotdev/vector/issues/5607
        //#[cfg(feature = "stdlib_only_fields")]
        //Box::new(OnlyFields),
        #[cfg(feature = "stdlib_object")]
        Box::new(Object),
        #[cfg(feature = "stdlib_parse_apache_log")]
        Box::new(ParseApacheLog),
        #[cfg(feature = "stdlib_parse_aws_alb_log")]
        Box::new(ParseAwsAlbLog),
        #[cfg(feature = "stdlib_parse_aws_cloudwatch_log_subscription_message")]
        Box::new(ParseAwsCloudWatchLogSubscriptionMessage),
        #[cfg(feature = "stdlib_parse_aws_vpc_flow_log")]
        Box::new(ParseAwsVpcFlowLog),
        #[cfg(feature = "stdlib_parse_cef")]
        Box::new(ParseCef),
        #[cfg(feature = "stdlib_parse_common_log")]
        Box::new(ParseCommonLog),
        #[cfg(feature = "stdlib_parse_csv")]
        Box::new(ParseCsv),
        #[cfg(feature = "stdlib_parse_duration")]
        Box::new(ParseDuration),
        #[cfg(feature = "stdlib_parse_glog")]
        Box::new(ParseGlog),
        #[cfg(feature = "stdlib_parse_grok")]
        Box::new(ParseGrok),
        #[cfg(feature = "stdlib_parse_groks")]
        Box::new(ParseGroks),
        #[cfg(feature = "stdlib_parse_int")]
        Box::new(ParseInt),
        #[cfg(feature = "stdlib_parse_json")]
        Box::new(ParseJson),
        #[cfg(feature = "stdlib_parse_key_value")]
        Box::new(ParseKeyValue),
        #[cfg(feature = "stdlib_parse_klog")]
        Box::new(ParseKlog),
        #[cfg(feature = "stdlib_parse_linux_authorization")]
        Box::new(ParseLinuxAuthorization),
        #[cfg(feature = "stdlib_parse_logfmt")]
        Box::new(ParseLogFmt),
        #[cfg(feature = "stdlib_parse_nginx_log")]
        Box::new(ParseNginxLog),
        #[cfg(feature = "stdlib_parse_query_string")]
        Box::new(ParseQueryString),
        #[cfg(feature = "stdlib_parse_regex")]
        Box::new(ParseRegex),
        #[cfg(feature = "stdlib_parse_regex_all")]
        Box::new(ParseRegexAll),
        #[cfg(feature = "stdlib_parse_ruby_hash")]
        Box::new(ParseRubyHash),
        #[cfg(feature = "stdlib_parse_syslog")]
        Box::new(ParseSyslog),
        #[cfg(feature = "stdlib_parse_timestamp")]
        Box::new(ParseTimestamp),
        #[cfg(feature = "stdlib_parse_tokens")]
        Box::new(ParseTokens),
        #[cfg(feature = "stdlib_parse_url")]
        Box::new(ParseUrl),
        #[cfg(feature = "stdlib_parse_user_agent")]
        Box::new(ParseUserAgent),
        #[cfg(feature = "stdlib_parse_xml")]
        Box::new(ParseXml),
        #[cfg(feature = "stdlib_push")]
        Box::new(Push),
        #[cfg(feature = "stdlib_random_bool")]
        Box::new(RandomBool),
        #[cfg(feature = "stdlib_random_bytes")]
        Box::new(RandomBytes),
        #[cfg(feature = "stdlib_random_float")]
        Box::new(RandomFloat),
        #[cfg(feature = "stdlib_random_int")]
        Box::new(RandomInt),
        #[cfg(feature = "stdlib_redact")]
        Box::new(Redact),
        #[cfg(feature = "stdlib_remove")]
        Box::new(Remove),
        #[cfg(feature = "stdlib_replace")]
        Box::new(Replace),
        #[cfg(feature = "stdlib_reverse_dns")]
        Box::new(ReverseDns),
        #[cfg(feature = "stdlib_round")]
        Box::new(Round),
        #[cfg(feature = "stdlib_seahash")]
        Box::new(Seahash),
        #[cfg(feature = "stdlib_set")]
        Box::new(Set),
        #[cfg(feature = "stdlib_sha1")]
        Box::new(Sha1),
        #[cfg(feature = "stdlib_sha2")]
        Box::new(Sha2),
        #[cfg(feature = "stdlib_sha3")]
        Box::new(Sha3),
        #[cfg(feature = "stdlib_slice")]
        Box::new(Slice),
        #[cfg(feature = "stdlib_split")]
        Box::new(Split),
        #[cfg(feature = "stdlib_starts_with")]
        Box::new(StartsWith),
        #[cfg(feature = "stdlib_string")]
        Box::new(String),
        #[cfg(feature = "stdlib_strip_ansi_escape_codes")]
        Box::new(StripAnsiEscapeCodes),
        #[cfg(feature = "stdlib_strip_whitespace")]
        Box::new(StripWhitespace),
        #[cfg(feature = "stdlib_strlen")]
        Box::new(Strlen),
        #[cfg(feature = "stdlib_tally")]
        Box::new(Tally),
        #[cfg(feature = "stdlib_tally_value")]
        Box::new(TallyValue),
        #[cfg(feature = "stdlib_tag_types_externally")]
        Box::new(TagTypesExternally),
        #[cfg(feature = "stdlib_timestamp")]
        Box::new(Timestamp),
        #[cfg(feature = "stdlib_to_bool")]
        Box::new(ToBool),
        #[cfg(feature = "stdlib_to_float")]
        Box::new(ToFloat),
        #[cfg(feature = "stdlib_to_int")]
        Box::new(ToInt),
        #[cfg(feature = "stdlib_to_regex")]
        Box::new(ToRegex),
        #[cfg(feature = "stdlib_to_string")]
        Box::new(ToString),
        #[cfg(feature = "stdlib_to_syslog_facility")]
        Box::new(ToSyslogFacility),
        #[cfg(feature = "stdlib_to_syslog_level")]
        Box::new(ToSyslogLevel),
        #[cfg(feature = "stdlib_to_syslog_severity")]
        Box::new(ToSyslogSeverity),
        #[cfg(feature = "stdlib_to_timestamp")]
        Box::new(ToTimestamp),
        #[cfg(feature = "stdlib_to_unix_timestamp")]
        Box::new(ToUnixTimestamp),
        #[cfg(feature = "stdlib_truncate")]
        Box::new(Truncate),
        #[cfg(feature = "stdlib_type_def")]
        Box::new(TypeDef),
        #[cfg(feature = "stdlib_unique")]
        Box::new(Unique),
        #[cfg(feature = "stdlib_unnest")]
        Box::new(Unnest),
        #[cfg(feature = "stdlib_upcase")]
        Box::new(Upcase),
        #[cfg(feature = "stdlib_uuid_v4")]
        Box::new(UuidV4),
        #[cfg(feature = "stdlib_values")]
        Box::new(Values),
    ]
}
