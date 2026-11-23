`ProgramInfo::target_queries` is now more precise for `get` and `unnest`. `get` no longer records a query for the whole event; the paths it reads, such as `.tags` in `get(.tags, path)`, are recorded from its arguments as for any other query. `unnest` now records the root of its target, because it copies the rest of the target around each element. Targets that prepare only the queried fields, such as Vector's metric events, no longer prepare the whole event for `get` calls, and keep the other fields when using `unnest`.

Functions can record additional paths they read with the new `FunctionCompileContext::record_target_query`. Hosts whose targets prepare only the queried fields should use it in custom functions that read the target directly instead of through their arguments. Previously, the root query from `get` could hide a missing query in programs that also called `get`; those functions may now read `null` until they record the paths they read.

authors: JinchengCao
