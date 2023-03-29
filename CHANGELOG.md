# Changelog

## unreleased
- added guard for the `limit` param of the `split` function to ensure it's not negative
- renamed `Expression::as_value` to `Expression::resolve_constant`
- `match` function now precompiles static regular expressions

## `0.1.0` (2023-03-27)
- VRL was split from the Vector repo
