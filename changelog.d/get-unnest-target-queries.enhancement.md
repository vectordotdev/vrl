`ProgramInfo::target_queries` is now more precise for `get` and `unnest`. `get` no longer records a query for the whole event; the paths it reads, such as `.tags` in `get(.tags, path)`, are recorded from its arguments as for any other query. `unnest` now records the root of its target, because it copies the rest of the target around each element. Targets that prepare only the queried fields, such as Vector's metric events, no longer prepare the whole event for `get` calls, and keep the other fields when using `unnest`.

Functions can record additional paths they read with the new `FunctionCompileContext::record_target_query`.

authors: JinchengCao
