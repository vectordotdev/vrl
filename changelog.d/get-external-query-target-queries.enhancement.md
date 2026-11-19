When the `value` argument of `get` is a path of the event or its metadata, such as `get(.tags, path)`, the compiler no longer records an extra query for the whole event in `ProgramInfo::target_queries`. Targets that prepare only the queried fields, such as Vector's metric events, no longer prepare the whole event for these calls.

authors: JinchengCao
