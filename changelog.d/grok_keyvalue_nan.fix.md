The `keyvalue` filter in `parse_grok` and `parse_groks` no longer panics on unquoted `NaN` values
such as `ratio=NaN`. These values are now kept as strings.

authors: tessneau
