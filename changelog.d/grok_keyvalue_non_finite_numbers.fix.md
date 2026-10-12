The `keyvalue` filter in `parse_grok` and `parse_groks` no longer panics on unquoted `NaN` values
such as `ratio=NaN`. Non-finite values (`NaN`, `inf`, `Infinity`, or numbers that overflow, such as
`1e999`) are now kept as strings instead of being parsed as numbers.

authors: tessneau
