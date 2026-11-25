`match_datadog_query` now matches attribute and tag wildcards with `*` only at the start or end (`foo*`, `*foo`, `*foo*`) using string comparisons instead of a regex. Results are unchanged. The new `WildcardMatcher` in `datadog_filter::regex` exposes the same matching to other `Filter` implementations.

authors: gwenaskell
