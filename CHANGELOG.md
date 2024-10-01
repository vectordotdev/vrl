# Changelog

 This project uses [*towncrier*](https://towncrier.readthedocs.io/) for changelog generation.

<!-- changelog start -->

## [0.19.0 (2024-09-30)]


### Breaking Changes & Upgrade Guide

- The multi-line mode of the `parse_groks` VRL function is now enabled by default.
  Use the `(?-m)` modifier to disable this behaviour. (https://github.com/vectordotdev/vrl/pull/1022)

### Enhancements

- The `keyvalue` grok filter is extended to match Datadog implementation. (https://github.com/vectordotdev/vrl/pull/1015)

### Fixes

- The `parse_xml` function now doesn't add an unnecessary `text` key when processing single nodes. (https://github.com/vectordotdev/vrl/pull/849)
- `parse_grok` and `parse_groks` no longer require field names containing a hyphen (e.g. `@a-b`) to be quoted.
- The function `match_datadog_query` doesn't panic if an invalid path is passed, instead it returns an error. (https://github.com/vectordotdev/vrl/pull/1031)
- The `parse_ruby_hash` parser is extended to match Datadog implementation. Previously it would parse the key in `{:key => "value"}` as `:key`, now it will parse it as `key`. (https://github.com/vectordotdev/vrl/pull/1050)


## [0.18.0 (2024-09-05)]


### New Features

- Added `unflatten` function to inverse the result of the `flatten` function. This function is useful when you want to convert a flattened object back to its original form.
- The `parse_json` function now accepts an optional `lossy` parameter (which defaults to `true`).

  This new parameter allows to control whether the UTF-8 decoding should be lossy or not, replacing
  invalid UTF-8 sequences with the Unicode replacement character (U+FFFD) if set to `true` or raising an error
  if set to `false` and an invalid utf-8 sequence is found. (https://github.com/vectordotdev/vrl/pull/269)
- Added casing functions `camelcase`, `kebabcase`, `screamingsnakecase`, `snakecase`, `pascalcase` (https://github.com/vectordotdev/vrl/pull/973)
- Added `parse_influxdb` function to parse events encoded using the [InfluxDB line protocol](https://docs.influxdata.com/influxdb/cloud/reference/syntax/line-protocol/).

### Enhancements

- The `match_datadog_query` function now accepts `||` in place of `OR` and `&&` in
  place of `AND` in the query string, which is common Datadog syntax. (https://github.com/vectordotdev/vrl/pull/1001)

### Fixes

- `decode_base64` no longer requires canonical padding. (https://github.com/vectordotdev/vrl/pull/960)
- The assumption of a Datadog Logs-based intake event structure has been removed
  from the `match_datadog_query` function. (https://github.com/vectordotdev/vrl/pull/1003)
- For the `parse_influxdb` function the `timestamp` and `tags` fields of returned objects are now
  correctly marked as nullable.

## [0.17.0 (2024-07-24)]


### Breaking Changes & Upgrade Guide

- `parse_logfmt` now processes 3 escape sequences when parsing: `\n`, `\"` and `\\`. This means that for example, `\n` in the input will be replaced with an actual newline character in parsed keys or values. (https://github.com/vectordotdev/vrl/pull/777)


## [0.16.1 (2024-07-08)]

### Enhancements

- `server` option for `dns_lookup` now properly replaces default server settings (https://github.com/vectordotdev/vrl/pull/910/files)

## [0.16.0 (2024-06-06)]


### Breaking Changes & Upgrade Guide

- The deprecated coalesce paths (i.e. `(field1|field2)`) feature is now removed. (https://github.com/vectordotdev/vrl/pull/836)

### New Features

- Added experimental `dns_lookup` function. It should be used with caution, since it involves network
  calls and is therefore very slow.

- Added `psl` argument to the `parse_etld` function. It enables customizing used public suffix list. If none is provided the default (https://publicsuffix.org/list/public_suffix_list.dat) is used, which is that was used before this change.

### Enhancements

- Add traceability_id field support to parse_aws_alb_log (https://github.com/vectordotdev/vrl/pull/862)


## [0.15.0 (2024-05-01)]


### Deprecations

- Coalesce paths (i.e. `(field1|field2)`) are deprecated and will be removed in a
  future version.  This feature is rarely used and not very useful. (https://github.com/vectordotdev/vrl/pull/815)


## [0.14.0 (2024-04-29)]


### New Features

- Add `uuid_from_friendly_id` for converting base62-encoded 128-bit identifiers to the hyphenated UUID format (https://github.com/vectordotdev/vrl/pull/803)

### Fixes

- `parse_json` now supports round-tripable float parsing by activating the `float_roundtrip` feature in serde_json (https://github.com/vectordotdev/vrl/pull/755)


## [0.13.0 (2024-03-18)]


### Breaking Changes & Upgrade Guide

- fixed `parse_logfmt` handling of escapes in values that could cause spurious keys to be created. As a result of this fix, the breaking change has been made to no longer allow empty keys in key-value pair formats (https://github.com/vectordotdev/vrl/pull/725)

### New Features

- Added the `return` expression as per [RFC 7496](https://github.com/vectordotdev/vector/blob/4671ccbf0a6359ef8b752fa99fae9eb9c60fdee5/rfcs/2023-02-08-7496-vrl-return.md). This expression can be used to terminate the VRL program early while still emitting a value. (https://github.com/vectordotdev/vrl/pull/712)
- Added `sieve` string function, which can remove unwanted characters from a string using a regex of
  allowed patterns. (https://github.com/vectordotdev/vrl/pull/724)
- Add VRL function `uuid_v7` that generates UUIDv7 timestamp-based unique identifiers. (https://github.com/vectordotdev/vrl/pull/738)
- Added `encode_proto` and `parse_proto` functions, which can be used to encode and decode protobufs.

  `parse_proto` accepts a bytes value, a proto descriptor file path and a message type and returns the VRL value as parsed from the proto. `encode_proto` does the reverse and converts a VRL value into a protobuf bytes value. (https://github.com/vectordotdev/vrl/pull/739)

### Fixes

- `parse_nginx` now accepts empty values for http referer (https://github.com/vectordotdev/vrl/pull/643)


## [0.12.0 (2024-03-08)]


### New Features

- Added `validate` option to `encode_punycode` and `decode_punycode`, which defaults to true, but can
  be used to skip validation when set to false. (https://github.com/vectordotdev/vrl/pull/709)


## [0.11.0 (2024-02-07)]


### New Features

- Added `parse_etld` function for extracting eTLD and eTLD+1 (https://github.com/vectordotdev/vrl/pull/669)
- Added `encode_punycode` and `decode_punycode` functions (https://github.com/vectordotdev/vrl/pull/672)

### Enhancements

- Introduced a `redactor` option in `redact` function to enable the substitution of redacted content with either a custom string or a hash representation. (https://github.com/vectordotdev/vrl/pull/633)
- Add VRL function `get_timezone_name` to return the configured/resolved IANA timezone name.

### Fixes

- Fixed a bug in exporting paths containing more than one "coalesce" segment. (https://github.com/vectordotdev/vrl/pull/679)


## [0.10.0 (2024-01-24)]


### New Features

- Introduced an unused expression checker. It's designed to detect and report unused expressions,
  helping users to clean up and optimize their VRL scripts. Note that this checker will not catch everything,
  but it does aim to eliminate false positives. For example, shadowed variables are not reported as unused.
  (https://github.com/vectordotdev/vrl/pull/622)
- Add a `replace_with` function that is similar to `replace` but takes a closure instead of a
  replacement string. (https://github.com/vectordotdev/vrl/pull/628)

### Enhancements

- Added the `alias_sources` parameter for `parse_groks` to read sources from files. (https://github.com/vectordotdev/vrl/pull/194)


## `0.9.1` (2023-12-21)

#### Bug Fixes
* Support for WASM features using `chrono` was readded. This was accidentally dropped in 0.9.0.

## `0.9.0` (2023-12-12)
* `parse_regex_all` `pattern` param  can now be resolved from a variable
* fixed `parse_json` data corruption issue for numbers greater or equal to `i64::MAX`
* support timestamp comparison using operators <, <=, >, >=

## `0.8.0` (2023-10-31)

#### Features
- added `contains_all` function (https://github.com/vectordotdev/vrl/pull/468)
- `from_unix_timestamp` now accepts a new unit: Microseconds. (https://github.com/vectordotdev/vrl/pull/492)
- `parse_nginx_log` no longer fails if `upstream_response_length`, `upstream_response_time`, `upstream_status` are missing (https://github.com/vectordotdev/vrl/pull/498)
- added `parse_float` function (https://github.com/vectordotdev/vrl/pull/484)
- improved fallibility diagnostics (https://github.com/vectordotdev/vrl/pull/523)
- added `encode_snappy` and `decode_snappy` functions (https://github.com/vectordotdev/vrl/pull/543)

## `0.7.0` (2023-09-25)

#### Bug Fixes
- `parse_nginx_log` doesn't fail if the values of key-value pairs in error logs is missing (https://github.com/vectordotdev/vrl/pull/442)
- `encode_gzip` and `encode_zlib` now correctly check the compression level (preventing a panic) (https://github.com/vectordotdev/vrl/pull/393)
- fix the type definition of array/object literal expressions where one of the values is undefined (https://github.com/vectordotdev/vrl/pull/401)
- `parse_aws_vpc_flow_log` now handles account-id value as a string, avoiding loss of leading zeros and case where value is `unknown` (https://github.com/vectordotdev/vrl/issues/263)

#### Features
- `parse_key_value` can now parse values enclosed in single quote characters (https://github.com/vectordotdev/vrl/pull/382)
- added `pretty` parameter for `encode_json` vrl function to produce pretty-printed JSON string (https://github.com/vectordotdev/vrl/pull/370)
- added `community_id` function for generation of [V1 Community IDs](https://github.com/corelight/community-id-spec) (https://github.com/vectordotdev/vrl/pull/360)
- updated aws vpc flow log parsing to include version 5 fields (https://github.com/vectordotdev/vrl/issues/227)
- removed deprecated `to_timestamp` function (https://github.com/vectordotdev/vrl/pull/452)
- changed `truncate` arguments, it now accepts a suffix string instead of a boolean (https://github.com/vectordotdev/vrl/pull/454)

## `0.6.0` (2023-08-02)

#### Bug Fixes

- enquote values containing `=` in `encode_logfmt` vrl function (https://github.com/vectordotdev/vector/issues/17855)
- breaking change to `parse_nginx_log()` to make it compatible to more unstandardized events (https://github.com/vectordotdev/vrl/pull/249)

#### Features

- deprecated `to_timestamp` vrl function (https://github.com/vectordotdev/vrl/pull/285)
- add support for chacha20poly1305, xchacha20poly1305, xsalsa20poly1305 algorithms for encryption/decryption (https://github.com/vectordotdev/vrl/pull/293)
- add support for resolving variables to `Expr::resolve_constant` (https://github.com/vectordotdev/vrl/pull/304)
- introduce new encryption/decryption algorithm options (`"AES-*-CTR-BE"`, `"AES-*-CTR-LE"`) https://github.com/vectordotdev/vrl/pull/299

## `0.5.0` (2023-06-28)
- added \0 (null) character literal to lex parser (https://github.com/vectordotdev/vrl/pull/259)
- added the `timezone` argument to the `format_timestamp` vrl function. (https://github.com/vectordotdev/vrl/pull/247)
- removed feature flags for each individual VRL function. (https://github.com/vectordotdev/vrl/pull/251)
- fixed a panic when arithmetic overflows. It now always wraps (only in debug builds). (https://github.com/vectordotdev/vrl/pull/252)
- `ingress_upstreaminfo` log format has been added to `parse_nginx_log` function (https://github.com/vectordotdev/vrl/pull/193)
- fixed type definitions for side-effects inside of queries (https://github.com/vectordotdev/vrl/pull/258)
- replaced `Program::final_type_state` with `Program::final_type_info` to give access to the type definitions of both the target and program result (https://github.com/vectordotdev/vrl/pull/262)
- added `from_unix_timestamp` vrl function (https://github.com/vectordotdev/vrl/pull/277)

## `0.4.0` (2023-05-11)
- consolidated all crates into the root `vrl` crate. The external API stayed the same, with the exception of macros, which are now all exported at the root of the `vrl` crate.
- published VRL to crates.io. Standard crate versioning will now be used instead of git tags.

## `0.3.0` (2023-05-05)
- fixed a type definition bug for assignments where the right-hand side of the assignment expression resolved to the `never` type
- removed the deprecated `FieldBuf` from `Field`
- removed the lookup v1 code
- renamed the `lookup` crate to `path`
- re-exported all sub-crates in the root `vrl` crate
- fix the `value` macro so it works when re-exported

## `0.2.0` (2023-04-03)
- added guard for the `limit` param of the `split` function to ensure it's not negative
- renamed `Expression::as_value` to `Expression::resolve_constant`
- `match` function now precompiles static regular expressions
- enabled the `encrypt` and `decrypt` VRL functions on the WASM playground
- update default branch to `main`
- the following VRL functions now compile on WASM (but abort at runtime)
  - `get_hostname`
  - `log'
  - `reverse_dns'
  - `parse_grok`
  - `parse_groks`

## `0.1.0` (2023-03-27)
- VRL was split from the Vector repo
