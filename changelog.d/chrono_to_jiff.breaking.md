# Timestamps use `jiff` instead of `chrono` {#chrono-to-jiff}

## Summary

VRL now uses [`jiff`](https://docs.rs/jiff) for timestamps and time zones, because `chrono` and
`chrono-tz` are unmaintained ([RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543),
[RUSTSEC-2026-0555](https://rustsec.org/advisories/RUSTSEC-2026-0555)).

Format strings keep their [chrono specifiers](https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers),
and parsed and formatted values are unchanged, with these exceptions:

- Timestamp parse error messages are different.
- Some inputs that failed before now parse: negative `%s`, fewer digits than `%.3f`/`%.6f`/`%.9f`
  require, `%I` without `%p`, and specifiers that only `jiff` supports, such as `%Q` and `%N`.
- Years must be in the range `-9999..=9999`. A leap second (`:60`) parses as `:59`.
- With the `local` time zone, `%Z` formats the abbreviation (for example `CET`) instead of the offset.
- The `local` time zone is cached for up to 5 minutes, so a change to `TZ` or `/etc/localtime`
  takes effect later.

## Migration

Rust users must update to the new types:

- `Value::Timestamp` holds a `jiff::Timestamp` instead of a `chrono::DateTime<Utc>`.
- `TimeZone::Named` holds a `jiff::tz::TimeZone` instead of a `chrono_tz::Tz`. `TimeZone` is no
  longer `Copy`, so `Runtime::resolve` and the conversion helpers take it by reference.
- `TimeZone::datetime_from_str` returns a `jiff::Timestamp`. Use `vrl::compiler::datetime` to
  format and parse with chrono-compatible format strings.
- `From<TimeZone> for chrono::FixedOffset` is removed. Use `TimeZone::to_jiff` instead.

authors: thomasqueirozb
