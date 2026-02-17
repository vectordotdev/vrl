#![deny(warnings, clippy::pedantic)]
pub use wasm_unsupported_function::WasmUnsupportedFunction;

use crate::compiler::Function;

mod ip_utils;
mod json_utils;
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
        mod basename;
        mod boolean;
        mod ceil;
        mod casing;
        mod chunks;
        mod compact;
        mod contains;
        mod contains_all;
        mod crc;
        mod decode_base16;
        mod decode_base64;
        mod decode_charset;
        mod decode_gzip;
        mod decode_lz4;
        mod decode_mime_q;
        mod decode_percent;
        mod decode_punycode;
        mod decode_snappy;
        mod decode_zlib;
        mod decode_zstd;
        mod decrypt;
        mod decrypt_ip;
        mod del;
        mod dirname;
        #[cfg(feature = "enable_network_functions")]
        mod dns_lookup;
        mod downcase;
        mod encode_base16;
        mod encode_base64;
        mod encode_charset;
        mod encode_gzip;
        mod encode_lz4;
        mod encode_json;
        mod encode_key_value;
        mod encode_logfmt;
        mod encode_percent;
        #[cfg(feature = "enable_system_functions")]
        mod encode_proto;
        mod encode_punycode;
        mod encode_snappy;
        mod encode_zlib;
        mod encode_zstd;
        mod encrypt;
        mod encrypt_ip;
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
        #[cfg(feature = "enable_env_functions")]
        mod get_env_var;
        #[cfg(feature = "enable_system_functions")]
        mod get_hostname;
        #[cfg(feature = "enable_system_functions")]
        mod get_timezone_name;
        mod haversine;
        mod hmac;
        #[cfg(feature = "enable_network_functions")]
        mod http_request;
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
        mod object_from_array;
        mod parse_apache_log;
        mod parse_aws_alb_log;
        mod parse_aws_cloudwatch_log_subscription_message;
        mod parse_aws_vpc_flow_log;
        mod parse_bytes;
        mod parse_cef;
        mod parse_cbor;
        mod parse_common_log;
        mod parse_csv;
        mod parse_duration;
        #[cfg(feature = "enable_system_functions")]
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
        #[cfg(feature = "enable_system_functions")]
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
        mod pop;
        mod push;
        mod random_bool;
        mod random_bytes;
        mod random_float;
        mod random_int;
        mod redact;
        mod remove;
        mod replace;
        mod replace_with;
        #[cfg(feature = "enable_network_functions")]
        mod reverse_dns;
        mod round;
        mod seahash;
        mod set;
        mod sha1;
        mod sha2;
        mod sha3;
        mod shannon_entropy;
        mod sieve;
        mod slice;
        mod split;
        mod split_path;
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
        mod to_syslog_facility_code;
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
        #[cfg(feature = "enable_system_functions")]
        mod validate_json_schema;
        mod xxhash;
        mod zip;

        // -----------------------------------------------------------------------------

        // Macro to keep pub use and all() function in sync
        macro_rules! stdlib_functions {
            (
                $(
                    $(#[$attr:meta])*
                    $path:path
                ),* $(,)?
            ) => {
                // Generate pub use statements
                $(
                    $(#[$attr])*
                    pub use $path;
                )*

                // Generate the all() function
                #[must_use]
                #[allow(clippy::too_many_lines)]
                pub fn all() -> Vec<Box<dyn Function>> {
                    vec![
                        $(
                            $(#[$attr])*
                            Box::new($path),
                        )*
                    ]
                }
            };
        }

        stdlib_functions! {
            self::hmac::Hmac,
            abs::Abs,
            append::Append,
            assert::Assert,
            assert_eq::AssertEq,
            basename::BaseName,
            boolean::Boolean,
            ceil::Ceil,
            chunks::Chunks,
            compact::Compact,
            contains::Contains,
            contains_all::ContainsAll,
            decode_base16::DecodeBase16,
            decode_base64::DecodeBase64,
            decode_charset::DecodeCharset,
            decode_gzip::DecodeGzip,
            decode_lz4::DecodeLz4,
            decode_mime_q::DecodeMimeQ,
            decode_percent::DecodePercent,
            decode_punycode::DecodePunycode,
            decode_snappy::DecodeSnappy,
            decode_zlib::DecodeZlib,
            decode_zstd::DecodeZstd,
            decrypt::Decrypt,
            decrypt_ip::DecryptIp,
            del::Del,
            dirname::DirName,
            #[cfg(feature = "enable_network_functions")]
            dns_lookup::DnsLookup,
            downcase::Downcase,
            casing::camelcase::Camelcase,
            casing::pascalcase::Pascalcase,
            casing::snakecase::Snakecase,
            casing::screamingsnakecase::ScreamingSnakecase,
            casing::kebabcase::Kebabcase,
            encode_base16::EncodeBase16,
            encode_base64::EncodeBase64,
            encode_charset::EncodeCharset,
            encode_gzip::EncodeGzip,
            encode_lz4::EncodeLz4,
            encode_json::EncodeJson,
            encode_key_value::EncodeKeyValue,
            encode_logfmt::EncodeLogfmt,
            encode_percent::EncodePercent,
            #[cfg(feature = "enable_system_functions")]
            encode_proto::EncodeProto,
            encode_punycode::EncodePunycode,
            encode_snappy::EncodeSnappy,
            encode_zlib::EncodeZlib,
            encode_zstd::EncodeZstd,
            encrypt::Encrypt,
            encrypt_ip::EncryptIp,
            ends_with::EndsWith,
            exists::Exists,
            filter::Filter,
            find::Find,
            flatten::Flatten,
            float::Float,
            floor::Floor,
            for_each::ForEach,
            format_int::FormatInt,
            format_number::FormatNumber,
            format_timestamp::FormatTimestamp,
            from_unix_timestamp::FromUnixTimestamp,
            self::community_id::CommunityID,
            get::Get,
            #[cfg(feature = "enable_env_functions")]
            get_env_var::GetEnvVar,
            #[cfg(feature = "enable_system_functions")]
            get_hostname::GetHostname,
            #[cfg(feature = "enable_system_functions")]
            get_timezone_name::GetTimezoneName,
            #[cfg(feature = "enable_network_functions")]
            http_request::HttpRequest,
            haversine::Haversine,
            includes::Includes,
            integer::Integer,
            ip_aton::IpAton,
            ip_cidr_contains::IpCidrContains,
            ip_ntoa::IpNtoa,
            ip_ntop::IpNtop,
            ip_pton::IpPton,
            ip_subnet::IpSubnet,
            ip_to_ipv6::IpToIpv6,
            ipv6_to_ipv4::Ipv6ToIpV4,
            is_array::IsArray,
            is_boolean::IsBoolean,
            is_empty::IsEmpty,
            is_float::IsFloat,
            is_integer::IsInteger,
            is_ipv4::IsIpv4,
            is_ipv6::IsIpv6,
            is_json::IsJson,
            is_null::IsNull,
            is_nullish::IsNullish,
            is_object::IsObject,
            is_regex::IsRegex,
            is_string::IsString,
            is_timestamp::IsTimestamp,
            join::Join,
            keys::Keys,
            length::Length,
            log::Log,
            map_keys::MapKeys,
            map_values::MapValues,
            match_any::MatchAny,
            match_array::MatchArray,
            match_datadog_query::MatchDatadogQuery,
            merge::Merge,
            mod_func::Mod,
            now::Now,
            object::Object,
            object_from_array::ObjectFromArray,
            parse_apache_log::ParseApacheLog,
            parse_aws_alb_log::ParseAwsAlbLog,
            parse_aws_cloudwatch_log_subscription_message::ParseAwsCloudWatchLogSubscriptionMessage,
            parse_aws_vpc_flow_log::ParseAwsVpcFlowLog,
            parse_bytes::ParseBytes,
            parse_cbor::ParseCbor,
            parse_cef::ParseCef,
            parse_common_log::ParseCommonLog,
            parse_csv::ParseCsv,
            parse_duration::ParseDuration,
            parse_float::ParseFloat,
            #[cfg(feature = "enable_system_functions")]
            parse_etld::ParseEtld,
            parse_glog::ParseGlog,
            parse_grok::ParseGrok,
            parse_groks::ParseGroks,
            parse_influxdb::ParseInfluxDB,
            parse_int::ParseInt,
            parse_json::ParseJson,
            parse_key_value::ParseKeyValue,
            parse_klog::ParseKlog,
            parse_linux_authorization::ParseLinuxAuthorization,
            parse_logfmt::ParseLogFmt,
            parse_nginx_log::ParseNginxLog,
            #[cfg(feature = "enable_system_functions")]
            parse_proto::ParseProto,
            parse_query_string::ParseQueryString,
            parse_regex::ParseRegex,
            parse_regex_all::ParseRegexAll,
            parse_ruby_hash::ParseRubyHash,
            parse_syslog::ParseSyslog,
            parse_timestamp::ParseTimestamp,
            parse_tokens::ParseTokens,
            parse_url::ParseUrl,
            parse_user_agent::ParseUserAgent,
            parse_xml::ParseXml,
            pop::Pop,
            push::Push,
            r#match::Match,
            random_bool::RandomBool,
            random_bytes::RandomBytes,
            random_float::RandomFloat,
            random_int::RandomInt,
            redact::Redact,
            remove::Remove,
            replace::Replace,
            replace_with::ReplaceWith,
            #[cfg(feature = "enable_network_functions")]
            reverse_dns::ReverseDns,
            round::Round,
            set::Set,
            sha2::Sha2,
            sha3::Sha3,
            shannon_entropy::ShannonEntropy,
            sieve::Sieve,
            slice::Slice,
            split::Split,
            split_path::SplitPath,
            starts_with::StartsWith,
            string::String,
            strip_ansi_escape_codes::StripAnsiEscapeCodes,
            strip_whitespace::StripWhitespace,
            strlen::Strlen,
            tag_types_externally::TagTypesExternally,
            tally::Tally,
            tally_value::TallyValue,
            timestamp::Timestamp,
            to_bool::ToBool,
            to_float::ToFloat,
            to_int::ToInt,
            to_regex::ToRegex,
            to_string::ToString,
            to_syslog_facility_code::ToSyslogFacilityCode,
            to_syslog_facility::ToSyslogFacility,
            to_syslog_level::ToSyslogLevel,
            to_syslog_severity::ToSyslogSeverity,
            to_unix_timestamp::ToUnixTimestamp,
            truncate::Truncate,
            type_def::TypeDef,
            unflatten::Unflatten,
            unique::Unique,
            unnest::Unnest,
            upcase::Upcase,
            uuid_from_friendly_id::UuidFromFriendlyId,
            uuid_v4::UuidV4,
            uuid_v7::UuidV7,
            values::Values,
            #[cfg(feature = "enable_system_functions")]
            validate_json_schema::ValidateJsonSchema,
            zip::Zip,
            self::array::Array,
            self::md5::Md5,
            self::seahash::Seahash,
            self::sha1::Sha1,
            self::xxhash::Xxhash,
            self::crc::Crc,
        }

        #[cfg(feature = "enable_system_functions")]
        pub use get_timezone_name::get_name_for_timezone;
    }
}
