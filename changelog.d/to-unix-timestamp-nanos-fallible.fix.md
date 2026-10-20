`to_unix_timestamp` with `unit: "nanoseconds"` is now correctly marked as fallible by the
compiler. Previously the compiler rejected error handling for this call as unnecessary,
even though the conversion can fail at runtime when the timestamp is out of range for a
64-bit nanosecond value. Programs using `to_unix_timestamp` with `unit: "nanoseconds"`
must now handle the error (for example with `!` or error coalescing). Other units are
unaffected.

authors: suryakant-source
