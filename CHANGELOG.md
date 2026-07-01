# Changelog

Changelog is generated from fragments in `changelog.d/` by the `release` crate.

<!-- changelog start -->

## [0.33.1 (2026-06-02)](https://github.com/vectordotdev/vrl/releases/tag/v0.33.1)

### Fixes

- Reverted `parse_regex` changes from 0.33.0 which introduced a performance regression in multi-threaded scenarios.

  [PR #1789](https://github.com/vectordotdev/vrl/pull/1789) by [@thomasqueirozb](https://github.com/thomasqueirozb)


## [0.33.0 (2026-05-28)](https://github.com/vectordotdev/vrl/releases/tag/v0.33.0)

### New Features

- VRL string literals now support `\u{HEX}` Unicode escape sequences. Any valid Unicode scalar value can be expressed, e.g. `"hello\u{1F30E}world"`. Invalid sequences (empty braces, non-hex digits, surrogate codepoints, or values above U+10FFFF) are reported as a compile-time error.

  [PR #1771](https://github.com/vectordotdev/vrl/pull/1771) by [@thomasqueirozb](https://github.com/thomasqueirozb)
- ~`parse_regex` now accepts dynamic regex patterns (variables and runtime expressions), consistent with `parse_regex_all`. When the pattern is a literal, return type information remains precise based on named capture groups.~

  ~[PR #1774](https://github.com/vectordotdev/vrl/pull/1774) by [@thomasqueirozb](https://github.com/thomasqueirozb)~

### Enhancements

- Updated user agent data for `parse_user_agent` function

  [PR #1776](https://github.com/vectordotdev/vrl/pull/1776) by [@JakubOnderka](https://github.com/JakubOnderka)
- Protobuf encoding now coerces compatible scalar types into the target field type: integers and strings are accepted for `bool` fields (using the same parsing as `to_bool`), and integers are accepted for `float`/`double` fields. Previously these inputs failed encoding and required explicit conversion in VRL.

  [PR #1763](https://github.com/vectordotdev/vrl/pull/1763) by [@flaviofcruz](https://github.com/flaviofcruz)
- Added an optional `allow_lossy_string_coercion` argument to `encode_proto`. VRL's protobuf encoding stringifies `Boolean`, `Integer`, `Float`, and `Timestamp` values when assigned to a protobuf `string` field as a convenience for callers handling loosely typed input. The [protobuf JSON mapping](https://protobuf.dev/programming-guides/json/) only accepts a JSON string for a `string` field, so callers who want strict spec-compliant encoding can now pass `allow_lossy_string_coercion: false`. The default stays `true`, so today's behavior is unchanged.

  [PR #1764](https://github.com/vectordotdev/vrl/pull/1764) by [@pront](https://github.com/pront)
- ~Improved performance of `parse_regex`/`parse_regex_all` by pre-computing capture group names and indices at compile time. Users may see anywhere from 4% to 13% speedups in some cases.~

  ~[PR #1773](https://github.com/vectordotdev/vrl/pull/1773) by [@thomasqueirozb](https://github.com/thomasqueirozb)~
- Improved performance of `parse_regex_all` by reusing the compiled regex across invocations.

  [PR #1775](https://github.com/vectordotdev/vrl/pull/1775) by [@thomasqueirozb](https://github.com/thomasqueirozb)

### Fixes

- The compiler now reports every unhandled-error in a single compilation pass instead of stopping at the first one. For example:

  ```coffee
  {
      push(.x, 1)
      .b = push(.y, 2)
  }
  ```

  now reports both `push(.x, 1)` (unhandled error) and `.b = push(.y, 2)` (unhandled fallible assignment) in one go. Previously you'd only see the second one, fix it, recompile, and only then discover the first.

  [PR #1759](https://github.com/vectordotdev/vrl/pull/1759) by [@pront](https://github.com/pront)
- Fixed a confusing compile error where a fallible call earlier in a block could cause a later, unrelated assignment to be reported as the problem. For example:

  ```coffee
  {
      .a = 1
      push(.x, 1)        # the unhandled error is actually here
      .b = 2             # but the compiler used to flag this line
  }
  ```

  The error is now reported on the actual fallible expression, so adding `!` or the `, err =` form fixes it where you'd expect. This also fixes the same shape inside closure bodies, e.g. inside `for_each`/`map_values`.

  [PR #453](https://github.com/vectordotdev/vrl/pull/453) by [@pront](https://github.com/pront)
- Fixed a false positive in the unused-variable diagnostic (`E900`) where a variable used before being reassigned (shadowed) was incorrectly flagged as unused at its original assignment.

  [PR #1743](https://github.com/vectordotdev/vrl/pull/1743) by [@pront](https://github.com/pront)
- `encode_proto` and `parse_proto` now support proto maps whose keys are integers or booleans, not just strings. Because VRL object keys are always strings, integer and boolean keys are written in their string form:

  ```coffee
  encode_proto({ "by_id": { "42": "alice" } }, "schema.desc", "MyMessage")
  ```

  Previously `parse_proto` errored on these maps and `encode_proto` silently dropped the field. Note that `encode_proto` will now return an error if a key string can't be parsed into the schema's key type (for example, `"abc"` against a `map<int32, ...>`).

  [PR #1762](https://github.com/vectordotdev/vrl/pull/1762) by [@flaviofcruz](https://github.com/flaviofcruz)
- Fixed a typo in enum variant that made it impossible to use `SCREAMING_SNAKE` in casing functions such as `pascalcase`, `camelcase` and others.

  `pascalcase("hello", original_case: "SCREAMING_SNAKE")` now compiles properly.

  [PR #1770](https://github.com/vectordotdev/vrl/pull/1770) by [@simplepad](https://github.com/simplepad)
- Allowed the `else` keyword (and `else if`) to appear on a new line after the closing `}` of an `if`-block. Previously the trailing newline terminated the if-expression at the parser level, forcing `else` to share a line with `}`.

  [PR #1756](https://github.com/vectordotdev/vrl/pull/1756) by [@pront](https://github.com/pront)


## [0.32.0 (2026-04-16)](https://github.com/vectordotdev/vrl/releases/tag/v0.32.0)

### New Features

- Added a new `encode_csv` function that encodes an array of values into a CSV-formatted string. This is the inverse of the existing `parse_csv` function and supports an optional single-byte delimiter (defaults to `,`).

  [PR #1649](https://github.com/vectordotdev/vrl/pull/1649) by [@armleth](https://github.com/armleth)
- Added `to_entries` and `from_entries` with jq-compatible behavior: `to_entries` supports both objects and arrays, and `from_entries` accepts `key`/`Key`/`name`/`Name` and `value`/`Value` aliases.

  [PR #1653](https://github.com/vectordotdev/vrl/pull/1653) by [@close2code-palm](https://github.com/close2code-palm)

### Enhancements

- Added `except` parameter to `flatten` function to exclude specific keys from being flattened.

  [PR #1682](https://github.com/vectordotdev/vrl/pull/1682) by [@benjamin-awd](https://github.com/benjamin-awd)

### Fixes

- Fixed a bug where the REPL input validator was executing programs instead of only compiling them, causing functions with side effects (e.g. `http_request`) to run twice per submission.

  [PR #1701](https://github.com/vectordotdev/vrl/pull/1701) by [@pront](https://github.com/pront)


## [0.31.0 (2026-03-05)](https://github.com/vectordotdev/vrl/releases/tag/v0.31.0)

### New Features

- Added a new `parse_yaml` function. This function parses yaml according to the [YAML 1.1 spec](https://yaml.org/spec/1.1/).

  [PR #1602](https://github.com/vectordotdev/vrl/pull/1602) by [@juchem](https://github.com/juchem)
- Added `--quiet` / `-q` flag to the CLI to suppress the banner text when starting the REPL.

  [PR #1617](https://github.com/vectordotdev/vrl/pull/1617) by [@thomasqueirozb](https://github.com/thomasqueirozb)

### Fixes

- Fixed a bug where lexer parse errors would emit a generic span with 202 error code instead of the
  proper error. Also fixed error positions from nested lexers (e.g., string literals inside function
  arguments) to correctly point to the actual location in the source.

  Before (generic E202 syntax error):

  ```
  $ string("\a")

  error[E202]: syntax error
    ┌─ :1:1
    │
  1 │ string("\a")
    │ ^^^^^^^^^^^^ unexpected error: invalid escape character: \a
    │
    = see language documentation at https://vrl.dev
    = try your code in the VRL REPL, learn more at https://vrl.dev/examples
  ```

  After (correct E209 invalid escape character):

  ```
  $ string("\a")

  error[E209]: invalid escape character: \a
    ┌─ :1:10
    │
  1 │ string("\a")
    │          ^ invalid escape character: a
    │
    = see language documentation at https://vrl.dev
    = try your code in the VRL REPL, learn more at https://vrl.dev/examples
  ```

  [PR #1579](https://github.com/vectordotdev/vrl/pull/1579) by [@thomasqueirozb](https://github.com/thomasqueirozb)
- Fixed a bug where `parse_duration` panicked when large values overflowed during multiplication.
  The function now returns an error instead.

  [PR #1618](https://github.com/vectordotdev/vrl/pull/1618) by [@thomasqueirozb](https://github.com/thomasqueirozb)
- Corrected the type definition of the `basename` function to indicate that it can also return `null`.
  Previously the type definitition indicated that the function could only return bytes (or strings).

  [PR #1635](https://github.com/vectordotdev/vrl/pull/1635) by [@thomasqueirozb](https://github.com/thomasqueirozb)
- Fixed incorrect parameter types in several stdlib functions:

  - `md5`: `value` parameter was typed as `any`, now correctly typed as `bytes`.
  - `seahash`: `value` parameter was typed as `any`, now correctly typed as `bytes`.
  - `floor`: `value` parameter was typed as `any`, now correctly typed as `float | integer`; `precision` parameter was typed as `any`, now correctly typed as `integer`.
  - `parse_key_value`: `key_value_delimiter` and `field_delimiter` parameters were typed as `any`, now correctly typed as `bytes`.

  Note: the function documentation already reflected the correct types.

  [PR #1650](https://github.com/vectordotdev/vrl/pull/1650) by [@thomasqueirozb](https://github.com/thomasqueirozb)


## [0.30.0 (2026-01-22)](https://github.com/vectordotdev/vrl/releases/tag/v0.30.0)

### Breaking Changes & Upgrade Guide

- The `usage()` method on the `Function` trait is now required. Custom VRL functions must implement this
  method to return a `&'static str` describing the function's purpose.

  [PR #1608](https://github.com/vectordotdev/vrl/pull/1608) by [@thomasqueirozb](https://github.com/thomasqueirozb)

### Fixes

- Corrected the type definition for `format_int` function to return bytes instead of integer.

  [PR #1586](https://github.com/vectordotdev/vrl/pull/1586) by [@thomasqueirozb](https://github.com/thomasqueirozb)


## [0.29.0 (2025-12-11)](https://github.com/vectordotdev/vrl/releases/tag/v0.29.0)

### Breaking Changes & Upgrade Guide

- Added required `line` and `file` fields to `vrl::compiler::function::Example`. Also added the
  `example!` macro to automatically populate those fields.

  [PR #1557](https://github.com/vectordotdev/vrl/pull/1557) by [@thomasqueirozb](https://github.com/thomasqueirozb)

### Fixes

- Fixed handling of OR conjunctions in the datadog search query parser

  [PR #1542](https://github.com/vectordotdev/vrl/pull/1542) by [@gwenaskell](https://github.com/gwenaskell)
- Fixed a bug where VRL would crash if `merge` were called without a `to` argument.

  [PR #1563](https://github.com/vectordotdev/vrl/pull/1563) by [@thomasqueirozb](https://github.com/thomasqueirozb)
- Fixed a bug where a stack overflow would happen in validate_json_schema if the schema had an empty $ref.

  [PR #1577](https://github.com/vectordotdev/vrl/pull/1577) by [@jlambatl](https://github.com/jlambatl)


## [0.28.1 (2025-11-07)](https://github.com/vectordotdev/vrl/releases/tag/v0.28.1)

### Fixes

- Fixed an issue where `split_path`, `basename`, `dirname` had not been added to VRL's standard
  library and, therefore, appeared to be missing and were inaccessible in the `0.28.0` release.

  [PR #1553](https://github.com/vectordotdev/vrl/pull/1553) by [@thomasqueirozb](https://github.com/thomasqueirozb)


## [0.28.0 (2025-11-03)](https://github.com/vectordotdev/vrl/releases/tag/v0.28.0)

### Breaking Changes & Upgrade Guide

- The return value of the `find` function has been changed to `null` instead of `-1` if there is no match.

  [PR #1514](https://github.com/vectordotdev/vrl/pull/1514) by [@titaneric](https://github.com/titaneric)

### New Features

- Introduced the `basename` function to get the last component of a path.

  [PR #1531](https://github.com/vectordotdev/vrl/pull/1531) by [@titaneric](https://github.com/titaneric)
- Introduced the `dirname` function to get the directory component of a path.

  [PR #1532](https://github.com/vectordotdev/vrl/pull/1532) by [@titaneric](https://github.com/titaneric)
- Introduced the `split_path` function to split a path into its components.

  [PR #1533](https://github.com/vectordotdev/vrl/pull/1533) by [@titaneric](https://github.com/titaneric)

### Enhancements

- Added optional `http_proxy` and `https_proxy` parameters to `http_request` for setting the proxies used for a request.

  [PR #1534](https://github.com/vectordotdev/vrl/pull/1534) by [@5Dev24](https://github.com/5Dev24)
- Added support for encoding a VRL `Integer` into a protobuf `double` when using `encode_proto`

  [PR #1545](https://github.com/vectordotdev/vrl/pull/1545) by [@thomasqueirozb](https://github.com/thomasqueirozb)

### Fixes

- Fixed `parse_glog` to accept space-padded thread-id.

  [PR #1515](https://github.com/vectordotdev/vrl/pull/1515) by [@suttod](https://github.com/suttod)


## [0.27.0 (2025-09-18)](https://github.com/vectordotdev/vrl/releases/tag/v0.27.0)

### Breaking Changes & Upgrade Guide

- The `validate_json_schema` functionality has been enhanced to collect and return validation error(s) in the error message return value, in addition to the existing primary Boolean `true / false` return value.

  Using JSON schema `test-schema.json` below:
  ```json
  {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "properties": {
      "test": {
        "type": "boolean"
      },
      "id": {
        "type": "integer"
      }
    },
    "required": ["test"],
    "additionalProperties": false
  }
  ```

  Before:
  ```
  $ invalid_object = { "id": "123" }
  { "id": "123" }

  $ valid, err = validate_json_schema(encode_json(invalid_object), "test-schema.json")
  false

  $ valid
  false

  $ err
  null
  ```

  After:
  ```
  $ invalid_object = { "id": "123" }
  { "id": "123" }

  $ valid, err = validate_json_schema(encode_json(invalid_object), "test-schema.json")
  "function call error for \"validate_json_schema\" at (13:82): JSON schema validation failed: \"123\" is not of type \"integer\" at /id, \"test\" is a required property at /"

  $ valid
  false

  $ err
  "function call error for \"validate_json_schema\" at (13:82): JSON schema validation failed: \"123\" is not of type \"integer\" at /id, \"test\" is a required property at /"
  ```

  [PR #1483](https://github.com/vectordotdev/vrl/pull/1483) by [@sbalmos](https://github.com/sbalmos)

### New Features

- Added a new `xxhash` function implementing `xxh32/xxh64/xxh3_64/xxh3_128` hashing algorithms.

  [PR #1473](https://github.com/vectordotdev/vrl/pull/1473) by [@stigglor](https://github.com/stigglor)
- Added an optional `strict_mode` parameter to `parse_aws_alb_log`. When set to `false`, the parser ignores any newly added/trailing fields in AWS ALB logs instead of failing. Defaults to `true` to preserve current behavior.

  [PR #1482](https://github.com/vectordotdev/vrl/pull/1482) by [@anas-aso](https://github.com/anas-aso)
- Added a new array function `pop` that removes the last item from an array.

  [PR #1501](https://github.com/vectordotdev/vrl/pull/1501) by [@jlambatl](https://github.com/jlambatl)
- Added two new cryptographic functions `encrypt_ip` and `decrypt_ip` for IP address encryption

  These functions use the IPCrypt specification and support both IPv4 and IPv6 addresses with two encryption modes: `aes128` (IPCrypt deterministic, 16-byte key) and `pfx` (IPCryptPfx, 32-byte key). Both algorithms are format-preserving (output is a valid IP address) and deterministic.

  [PR #1506](https://github.com/vectordotdev/vrl/pull/1506) by [@alterstep](https://github.com/alterstep)

### Enhancements

- Added an optional `body` parameter to `http_request`. Best used when sending a POST or PUT request.

  This does not perform automatic setting of `Content-Type` or `Content-Length` header(s). The caller should add these headers using the `headers` map parameter.

  [PR #1502](https://github.com/vectordotdev/vrl/pull/1502) by [@sbalmos](https://github.com/sbalmos)

### Fixes

- The `validate_json_schema` function no longer panics if the JSON schema file cannot be accessed or is invalid.

  [PR #1476](https://github.com/vectordotdev/vrl/pull/1476) by [@sbalmos](https://github.com/sbalmos)
- Fixed the `http_request` function's ability to run from the VRL CLI, no longer panics.

  [PR #1510](https://github.com/vectordotdev/vrl/pull/1510) by [@sbalmos](https://github.com/sbalmos)


## [0.26.0 (2025-08-07)](https://github.com/vectordotdev/vrl/releases/tag/v0.26.0)

### Breaking Changes & Upgrade Guide

- The `parse_cef` now trims unnecessary whitespace around escaped values in both headers and extension fields, improving accuracy and reliability when dealing with messy input strings.

  Scenario: `parse_cef` with whitespace post cef fields

  Previous Behavior: Runtime Error

  If an input with space added to parse_cef was provided, it would result in a runtime error due to the inability to parse the line successfully.
  Input: `CEF:1|Security|threatmanager|1.0|100|worm successfully stopped|10| dst=2.1.2.2 msg=Detected a threat. No action needed spt=1232`
  Output:
  ```
  error[E000]: function call error for "parse_cef" at (0:20): Could not parse whole line successfully
    ┌─ :1:1
    │.message = "CEF:1|Security|threatmanager|1.0|100|worm successfully stopped|10| dst=2.1.2.2 msg=Detected a threat. No action needed spt=1232"
  1 │ parse_cef!(.message)
    │ ^^^^^^^^^^^^^^^^^^^^ Could not parse whole line successfully
    │
    = see language documentation at https://vrl.dev
    = try your code in the VRL REPL, learn more at https://vrl.dev/examples
  ```

  New Behavior: parses data correctly

  ```
  {
      "cefVersion": "1",
      "deviceEventClassId": "100",
      "deviceProduct": "threatmanager",
      "deviceVendor": "Security",
      "deviceVersion": "1.0",
      "dst": "2.1.2.2",
      "msg": "Detected a threat. No action needed",
      "name": "worm successfully stopped",
      "severity": "10",
      "spt": "1232"
  }
  ```

  Scenario: `parse_cef` with whitespace in cef fields
  Input: `CEF:1|Security|threatmanager|1.0|100|worm successfully stopped|10| dst=2.1.2.2 msg=Detected a threat. No action needed  spt=1232`

  Previous Behavior: "msg": "Detected a threat. No action needed   "
  New Behavior: "msg": "Detected a threat. No action needed"

  [PR #1430](https://github.com/vectordotdev/vrl/pull/1430) by [@yjagdale](https://github.com/yjagdale)
- The `parse_syslog` function now treats RFC 3164 structured data items with no parameters (e.g., `[exampleSDID@32473]`) as part of the main
  message, rather than parsing them as structured data. Items with parameters (e.g., `[exampleSDID@32473 field="value"]`) continue to be
  parsed as structured data. (https://github.com/vectordotdev/vrl/pull/1435)
- `encode_lz4`  no longer prepends the uncompressed size by default, improving compatibility with standard LZ4 tools. A new `prepend_size` flag restores the old behavior if needed. Also, `decode_lz4` now also accepts `prepend_size` and a `buf_size` option (default: 1MB).

  Existing users of `encode_lz4` and `decode_lz4` will need to update their functions to include the argument `prepend_size: true` to maintain existing compatibility.

  [PR #1447](https://github.com/vectordotdev/vrl/pull/1447) by [@jlambatl](https://github.com/jlambatl)

### New Features

- Added `haversine` function for calculating [haversine](https://en.wikipedia.org/wiki/Haversine_formula) distance and bearing.

  [PR #1442](https://github.com/vectordotdev/vrl/pull/1442) by [@esensar](https://github.com/esensar), [@Quad9DNS](https://github.com/Quad9DNS)
- Add `validate_json_schema` function for validating JSON payloads against JSON schema files. A optional configuration parameter `ignore_unknown_formats` is provided to change how custom formats are handled by the validator. Unknown formats can be silently ignored by setting this to `true` and validation continues without failing due to those fields.

  [PR #1443](https://github.com/vectordotdev/vrl/pull/1443) by [@jlambatl](https://github.com/jlambatl)


## [0.25.0 (2025-06-26)](https://github.com/vectordotdev/vrl/releases/tag/v0.25.0)

### Enhancements

- Add support for decompressing lz4 frame compressed data.

  [PR #1367](https://github.com/vectordotdev/vrl/pull/1367) by [@jimmystewpot](https://github.com/jimmystewpot)


## [0.24.0 (2025-05-19)](https://github.com/vectordotdev/vrl/releases/tag/v0.24.0)

### Enhancements

- The `encode_gzip`, `decode_gzip`, `encode_zlib` and `decode_zlib` methods now uses the [zlib-rs](https://github.com/trifectatechfoundation/zlib-rs) backend
  which is much faster than the previous backend `miniz_oxide`.

  [PR #1301](https://github.com/vectordotdev/vrl/pull/1301) by [@JakubOnderka](https://github.com/JakubOnderka)
- The `decode_base64`, `encode_base64` and `decode_mime_q` functions now use the SIMD backend
  which is faster than the previous backend.

  [PR #1379](https://github.com/vectordotdev/vrl/pull/1379) by [@JakubOnderka](https://github.com/JakubOnderka)

### Fixes

- Add BOM stripping logic to the parse_json function.

  [PR #1370](https://github.com/vectordotdev/vrl/pull/1370) by [@thomasqueirozb](https://github.com/thomasqueirozb)


## [0.23.0 (2025-04-03)](https://github.com/vectordotdev/vrl/releases/tag/v0.23.0)


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

  [PR #1286](https://github.com/vectordotdev/vrl/pull/1286) by [@JakubOnderka](https://github.com/JakubOnderka)

### New Features

- Support for encoding and decoding lz4 block compression.

  [PR #1339](https://github.com/vectordotdev/vrl/pull/1339) by [@jimmystewpot](https://github.com/jimmystewpot)

### Enhancements

- The `encode_proto` function was enhanced to automatically convert integer, float, and boolean values when passed to string proto fields.

  [PR #1304](https://github.com/vectordotdev/vrl/pull/1304) by [@roykim98](https://github.com/roykim98)
- The `parse_user_agent` method now uses the [ua-parser](https://crates.io/crates/ua-parser) library
  which is much faster than the previous library. The method's output remains unchanged.

  [PR #1317](https://github.com/vectordotdev/vrl/pull/1317) by [@JakubOnderka](https://github.com/JakubOnderka)
- Added support for excluded_boundaries in the `snakecase()` function. This allows users to leverage the same function `snakecase()` that they're already leveraging but tune it to handle specific scenarios where default boundaries are not desired.

  For example,

  ```rust
  snakecase("s3BucketDetails", excluded_boundaries: ["digit_lower", "lower_digit", "upper_digit"])
  /// Output: s3_bucket_details
  ```

  [PR #1324](https://github.com/vectordotdev/vrl/pull/1324) by [@brittonhayes](https://github.com/brittonhayes)

### Fixes

- The `parse_nginx_log` function can now parse `delaying requests` error messages.

  [PR #1285](https://github.com/vectordotdev/vrl/pull/1285) by [@JakubOnderka](https://github.com/JakubOnderka)


## [0.22.0 (2025-02-19)](https://github.com/vectordotdev/vrl/releases/tag/v0.22.0)


### Breaking Changes & Upgrade Guide

- Removed deprecated `ellipsis` argument from the `truncate` function. Use `suffix` instead.

  [PR #1188](https://github.com/vectordotdev/vrl/pull/1188) by [@pront](https://github.com/pront)
- Fix `slice` type_def. This is a breaking change because it might change the fallibility of the `slice` function and this VRL scripts will
  need to be updated accordingly.

  [PR #1246](https://github.com/vectordotdev/vrl/pull/1246) by [@pront](https://github.com/pront)

### New Features

- Added new `to_syslog_facility_code` function to convert syslog facility keyword to syslog facility code.

  [PR #1221](https://github.com/vectordotdev/vrl/pull/1221) by [@simplepad](https://github.com/simplepad)
- Downgrade "can't abort infallible function" error to a warning.

  [PR #1247](https://github.com/vectordotdev/vrl/pull/1247) by [@pront](https://github.com/pront)
- `ip_cidr_contains` method now also accepts an array of CIDRs.

  [PR #1248](https://github.com/vectordotdev/vrl/pull/1248) by [@JakubOnderka](https://github.com/JakubOnderka)
- Faster converting bytes to Unicode string by using SIMD instructions provided by simdutf8 crate.
  simdutf8 is up to 23 times faster than the std library on valid non-ASCII, up to four times on pure
  ASCII is the same method provided by Rust's standard library. This will speed up almost all VRL methods
  like `parse_json` or `parse_regex`.

  [PR #1249](https://github.com/vectordotdev/vrl/pull/1249) by [@JakubOnderka](https://github.com/JakubOnderka)
- Added `shannon_entropy` function to generate [entropy](https://en.wikipedia.org/wiki/Entropy_(information_theory)) from a string.

  [PR #1267](https://github.com/vectordotdev/vrl/pull/1267) by [@esensar](https://github.com/esensar)

### Fixes

- Fix decimals parsing in parse_duration function

  [PR #1223](https://github.com/vectordotdev/vrl/pull/1223) by [@sainad2222](https://github.com/sainad2222)
- Fix `parse_nginx_log` function when a format is set to error and an error message contains comma.

  [PR #1280](https://github.com/vectordotdev/vrl/pull/1280) by [@JakubOnderka](https://github.com/JakubOnderka)


## [0.21.0 (2025-01-13)](https://github.com/vectordotdev/vrl/releases/tag/v0.21.0)


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


## [0.20.1 (2024-12-09)](https://github.com/vectordotdev/vrl/releases/tag/v0.20.1)


### Fixes

- Reverted `to_float` [change](https://github.com/vectordotdev/vrl/pull/1107) because the new logic is too restrictive
  e.g. attempting to convert "0" returns an error. (https://github.com/vectordotdev/vrl/pull/1179)


## [0.20.0 (2024-11-27)](https://github.com/vectordotdev/vrl/releases/tag/v0.20.0)


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


## [0.19.0 (2024-09-30)](https://github.com/vectordotdev/vrl/releases/tag/v0.19.0)


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


## [0.18.0 (2024-09-05)](https://github.com/vectordotdev/vrl/releases/tag/v0.18.0)


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

## [0.17.0 (2024-07-24)](https://github.com/vectordotdev/vrl/releases/tag/v0.17.0)


### Breaking Changes & Upgrade Guide

- `parse_logfmt` now processes 3 escape sequences when parsing: `\n`, `\"` and `\\`. This means that for example, `\n` in the input will be replaced with an actual newline character in parsed keys or values. (https://github.com/vectordotdev/vrl/pull/777)


## [0.16.1 (2024-07-08)](https://github.com/vectordotdev/vrl/releases/tag/v0.16.1)

### Enhancements

- `server` option for `dns_lookup` now properly replaces default server settings (https://github.com/vectordotdev/vrl/pull/910/files)

## [0.16.0 (2024-06-06)](https://github.com/vectordotdev/vrl/releases/tag/v0.16.0)


### Breaking Changes & Upgrade Guide

- The deprecated coalesce paths (i.e. `(field1|field2)`) feature is now removed. (https://github.com/vectordotdev/vrl/pull/836)

### New Features

- Added experimental `dns_lookup` function. It should be used with caution, since it involves network
  calls and is therefore very slow.

- Added `psl` argument to the `parse_etld` function. It enables customizing used public suffix list. If none is provided the default (https://publicsuffix.org/list/public_suffix_list.dat) is used, which is that was used before this change.

### Enhancements

- Add traceability_id field support to parse_aws_alb_log (https://github.com/vectordotdev/vrl/pull/862)


## [0.15.0 (2024-05-01)](https://github.com/vectordotdev/vrl/releases/tag/v0.15.0)


### Deprecations

- Coalesce paths (i.e. `(field1|field2)`) are deprecated and will be removed in a
  future version.  This feature is rarely used and not very useful. (https://github.com/vectordotdev/vrl/pull/815)


## [0.14.0 (2024-04-29)](https://github.com/vectordotdev/vrl/releases/tag/v0.14.0)


### New Features

- Add `uuid_from_friendly_id` for converting base62-encoded 128-bit identifiers to the hyphenated UUID format (https://github.com/vectordotdev/vrl/pull/803)

### Fixes

- `parse_json` now supports round-tripable float parsing by activating the `float_roundtrip` feature in serde_json (https://github.com/vectordotdev/vrl/pull/755)


## [0.13.0 (2024-03-18)](https://github.com/vectordotdev/vrl/releases/tag/v0.13.0)


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


## [0.12.0 (2024-03-08)](https://github.com/vectordotdev/vrl/releases/tag/v0.12.0)


### New Features

- Added `validate` option to `encode_punycode` and `decode_punycode`, which defaults to true, but can
  be used to skip validation when set to false. (https://github.com/vectordotdev/vrl/pull/709)


## [0.11.0 (2024-02-07)](https://github.com/vectordotdev/vrl/releases/tag/v0.11.0)


### New Features

- Added `parse_etld` function for extracting eTLD and eTLD+1 (https://github.com/vectordotdev/vrl/pull/669)
- Added `encode_punycode` and `decode_punycode` functions (https://github.com/vectordotdev/vrl/pull/672)

### Enhancements

- Introduced a `redactor` option in `redact` function to enable the substitution of redacted content with either a custom string or a hash representation. (https://github.com/vectordotdev/vrl/pull/633)
- Add VRL function `get_timezone_name` to return the configured/resolved IANA timezone name.

### Fixes

- Fixed a bug in exporting paths containing more than one "coalesce" segment. (https://github.com/vectordotdev/vrl/pull/679)


## [0.10.0 (2024-01-24)](https://github.com/vectordotdev/vrl/releases/tag/v0.10.0)


### New Features

- Introduced an unused expression checker. It's designed to detect and report unused expressions,
  helping users to clean up and optimize their VRL scripts. Note that this checker will not catch everything,
  but it does aim to eliminate false positives. For example, shadowed variables are not reported as unused.
  [PR #622](https://github.com/vectordotdev/vrl/pull/622)
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
