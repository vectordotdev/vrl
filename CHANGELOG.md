# Changelog

 This project uses [*towncrier*](https://towncrier.readthedocs.io/) for changelog generation.

<!-- changelog start -->

## [0.11.0 (2024-02-07)]


### New Features

- Added `parse_etld` function for extracting eTLD and eTLD+1 (https://github.com/vectordotdev/vrl/pull/669)
- Added `encode_punycode` and `decode_punycode` functions (https://github.com/vectordotdev/vrl/pull/672)

### Enhancements

- Introduced a `redactor` option in `redact` function to enable the substitution of redacted content with either a custom string or a hash representation. (https://github.com/vectordotdev/vrl/pull/633)
- Add VRL function `get_timezone_name` to return the configured/resolved IANA timezone name.

  authors: klondikedragon (https://github.com/vectordotdev/vrl/pull/671)

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
