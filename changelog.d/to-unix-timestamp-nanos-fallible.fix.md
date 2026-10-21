`to_unix_timestamp` with `unit: "nanoseconds"` is now correctly marked as fallible by the
compiler. The conversion can fail at runtime when the timestamp is out of range for a
64-bit nanosecond value, but the compiler previously rejected error handling for this
call as unnecessary.

Programs that call `to_unix_timestamp` with `unit: "nanoseconds"` are affected: they no
longer compile until the error is handled. Other units are unaffected.

**Before**

```vrl
to_unix_timestamp(ts, unit: "nanoseconds")
```

**After**

```vrl
to_unix_timestamp!(ts, unit: "nanoseconds")
```

or with error coalescing:

```vrl
to_unix_timestamp(ts, unit: "nanoseconds") ?? 0
```

authors: suryakant-source
