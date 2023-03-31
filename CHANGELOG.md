# Changelog

## unreleased
- added guard for the `limit` param of the `split` function to ensure it's not negative
- renamed `Expression::as_value` to `Expression::resolve_constant`
- `match` function now precompiles static regular expressions
- update default branch to `main`
- the following VRL functions now compile on WASM (but abort at runtime)
  - `get_hostname`
  - `reverse_dns'
  - `parse_grok`
  - `parse_groks`

## `0.1.0` (2023-03-27)
- VRL was split from the Vector repo
