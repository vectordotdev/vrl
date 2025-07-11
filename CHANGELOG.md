# Changelog

 This project uses [*towncrier*](https://towncrier.readthedocs.io/) for changelog generation.

<!-- changelog start -->

## [0.25.0 (2025-06-26)]

### Enhancements

- Add support for decompressing lz4 frame compressed data.

  authors: jimmystewpot (https://github.com/vectordotdev/vrl/pull/1367)


## [0.24.0 (2025-05-19)]

### Enhancements

- The `encode_gzip`, `decode_gzip`, `encode_zlib` and `decode_zlib` methods now uses the [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) backend
  which is much faster than the previous backend `miniz_oxide`.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1301)
- The `decode_base64`, `encode_base64` and `decode_mime_q` functions now use the SIMD backend
  which is faster than the previous backend.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1379)

### Fixes

- Add BOM stripping logic to the parse_json function.

  authors: thomasqueirozb (https://github.com/vectordotdev/vrl/pull/1370)


## [0.23.0 (2025-04-03)]


### Breaking Changes & Upgrade Guide

- The `ip_cidr_contains` function now validates the cidr argument during the compilation phase if it is a constant string or array. Previously, invalid constant CIDR values would only trigger an error during execution.

  Previous Behavior: Runtime Error

  Previously, if an invalid CIDR was passed as a constant, an error was thrown at runtime:

  ```
  error[E000]: function call error for "ip_cidr_contains" at (0:45): unable to parse CIDR: couldn't parse address in network: invalid IP address syntax
    ┌─ :1:1
    │
  1 │ ip_cidr_contains!("INVALID", "192.168.10.32")
    │ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unable to parse CIDR: couldn't parse address in network: invalid IP address syntax
    │
    = see language documentation at https://vrl.dev
    = try your code in the VRL REPL, learn more at https://vrl.dev/examples
  ```

  New Behavior: Compilation Error

  ```
  error[E610]: function compilation error: error[E403] invalid argument
    ┌─ :1:1
    │
  1 │ ip_cidr_contains!("INVALID", "192.168.10.32")
    │ ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    │ │
    │ invalid argument "ip_cidr_contains"
    │ error: "cidr" must be valid cidr
    │ received: "INVALID"
    │
    = learn more about error code 403 at https://errors.vrl.dev/403
    = see language documentation at https://vrl.dev
    = try your code in the VRL REPL, learn more at https://vrl.dev/examples
  ```

  This change improves error detection by identifying invalid CIDR values earlier, reducing unexpected failures at runtime and provides better performance.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1286)

### New Features

- Support for encoding and decoding lz4 block compression.

  authors: jimmystewpot (https://github.com/vectordotdev/vrl/pull/1339)

### Enhancements

- The `encode_proto` function was enhanced to automatically convert integer, float, and boolean values when passed to string proto fields. (https://github.com/vectordotdev/vrl/pull/1304)
- The `parse_user_agent` method now uses the [ua-parser](https://crates.io/crates/ua-parser) library
  which is much faster than the previous library. The method's output remains unchanged.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1317)
- Added support for excluded_boundaries in the `snakecase()` function. This allows users to leverage the same function `snakecase()` that they're already leveraging but tune it to handle specific scenarios where default boundaries are not desired.

  For example,

  ```rust
  snakecase("s3BucketDetails", excluded_boundaries: ["digit_lower", "lower_digit", "upper_digit"])
  /// Output: s3_bucket_details
  ```

  authors: brittonhayes (https://github.com/vectordotdev/vrl/pull/1324)

### Fixes

- The `parse_nginx_log` function can now parse `delaying requests` error messages.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1285)


## [0.22.0 (2025-02-19)]


### Breaking Changes & Upgrade Guide

- Removed deprecated `ellipsis` argument from the `truncate` function. Use `suffix` instead. (https://github.com/vectordotdev/vrl/pull/1188)
- Fix `slice` type_def. This is a breaking change because it might change the fallibility of the `slice` function and this VRL scripts will
  need to be updated accordingly.

  authors: pront (https://github.com/vectordotdev/vrl/pull/1246)

### New Features

- Added new `to_syslog_facility_code` function to convert syslog facility keyword to syslog facility code. (https://github.com/vectordotdev/vrl/pull/1221)
- Downgrade "can't abort infallible function" error to a warning. (https://github.com/vectordotdev/vrl/pull/1247)
- `ip_cidr_contains` method now also accepts an array of CIDRs.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1248)
- Faster converting bytes to Unicode string by using SIMD instructions provided by simdutf8 crate.
  simdutf8 is up to 23 times faster than the std library on valid non-ASCII, up to four times on pure
  ASCII is the same method provided by Rust's standard library. This will speed up almost all VRL methods
  like `parse_json` or `parse_regex`.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1249)
- Added `shannon_entropy` function to generate [entropy](https://en.wikipedia.org/wiki/Entropy_(information_theory)) from a string.

  authors: esensar (https://github.com/vectordotdev/vrl/pull/1267)

### Fixes

- Fix decimals parsing in parse_duration function

  authors: sainad2222 (https://github.com/vectordotdev/vrl/pull/1223)
- Fix `parse_nginx_log` function when a format is set to error and an error message contains comma.

  authors: JakubOnderka (https://github.com/vectordotdev/vrl/pull/1280)


## [0.21.0 (2025-01-13)]


### Breaking Changes & Upgrade Guide

- `to_unix_timestamp`, `to_float`, and `uuid_v7` can now return an error if the supplied timestamp is unrepresentable as a nanosecond timestamp. Previously the function calls would panic. (https://github.com/vectordotdev/vrl/pull/979)

### New Features

- Added new `crc` function to calculate CRC (Cyclic Redundancy Check) checksum
- Add `parse_cbor` function (https://github.com/vectordotdev/vrl/pull/1152)
- Added new `zip` function to iterate over an array of arrays and produce a new
  arrays containing an item from each one. (https://github.com/vectordotdev/vrl/pull/1158)
- Add new `decode_charset`, `encode_charset` functions to decode and encode strings between different charsets. (https://github.com/vectordotdev/vrl/pull/1162)
- Added new `object_from_array` function to create an object from an array of
  value pairs such as what `zip` can produce. (https://github.com/vectordotdev/vrl/pull/1164)
- Added support for multi-unit duration strings (e.g., `1h2s`, `2m3s`) in the `parse_duration` function. (https://github.com/vectordotdev/vrl/pull/1197)
- Added new `parse_bytes` function to parse given bytes string such as `1MiB` or `1TB` either in binary or decimal base. (https://github.com/vectordotdev/vrl/pull/1198)
- Add `main` log format for `parse_nginx_log`. (https://github.com/vectordotdev/vrl/pull/1202)
- Added support for optional `timezone` argument in the `parse_timestamp` function. (https://github.com/vectordotdev/vrl/pull/1207)

### Fixes

- Fix a panic in float subtraction that produces NaN values. (https://github.com/vectordotdev/vrl/pull/1186)


## [0.20.1 (2024-12-09)]


### Fixes

- Reverted `to_float` [change](https://github.com/vectordotdev/vrl/pull/1107) because the new logic is too restrictive
  e.g. attempting to convert "0" returns an error. (https://github.com/vectordotdev/vrl/pull/1179)


## [0.20.0 (2024-11-27)]


### Breaking Changes & Upgrade Guide

- Fixes the `to_float` function to return an error instead of `f64::INFINITY` when parsing [non-normal](https://doc.rust-lang.org/std/primitive.f64.html#method.is_normal) numbers. (https://github.com/vectordotdev/vrl/pull/1107)

### New Features

- The `decrypt` and `encrypt` VRL functions now support aes-siv (RFC 5297) encryption and decryption. (https://github.com/vectordotdev/vrl/pull/1100)

### Enhancements

- `decode_punycode` and `encode_punycode` with `validate` flag set to false should be faster now, in cases when input data needs no encoding or decoding. (https://github.com/vectordotdev/vrl/pull/1104)
- `vrl::value::Value` now implements `PartialCmp` that first checks whether the enum discriminants
  (that both are floats for example), and if they are calls `partial_cmp` on the inner values.
  Otherwise, it will return `None`. (https://github.com/vectordotdev/vrl/pull/1117)
- The `encode_proto` function was enhanced to automatically convert valid string fields to numeric proto
  fields. (https://github.com/vectordotdev/vrl/pull/1114)

### Fixes

- The `parse_groks` VRL function and Datadog grok parsing now catch the panic coming from `rust-onig` on too many regex match retries, and handles it as a custom error. (https://github.com/vectordotdev/vrl/pull/1079)
- `encode_punycode` with `validate` flag set to false should be more consistent with `validate` set to true, turning all uppercase character to lowercase besides doing punycode encoding (https://github.com/vectordotdev/vrl/pull/1115)
- Removed false warning when using `set_semantic_meaning`. (https://github.com/vectordotdev/vrl/pull/1148)


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
