`Value` now has a `String` variant (`bytestring::ByteString`) for guaranteed UTF-8, alongside
`Bytes` for raw byte sequences. VRL `==` / `Hash` / ordering still compare byte content across the
two variants. Natural UTF-8 constructors (`From<&str>`, JSON strings, VRL string literals) now
produce `Value::String`; `From<Bytes>` is unchanged.

This is a compile-time break for exhaustive `match` on `Value`. It is a **silent** skip for
non-exhaustive checks: `if let Value::Bytes(...)` and `matches!(..., Value::Bytes(_))` keep
compiling but miss VRL literals, JSON strings, and parser output.

```
// compile break
match value {
    Value::Bytes(b) => { /* ... */ }
    // other arms...
}

// silent skip of Value::String
if let Value::Bytes(b) = value { /* never runs for "foo" */ }

// accept both (preferred: accessors)
if value.is_bytes() {
    let _ = value.as_bytes();
    let _ = value.as_str();
}
```

Implements "Phase A" of [RFC 2026-05-01 - Add `Value::String` Variant for Guaranteed-UTF-8 Strings](rfcs/2026-05-01-value-string-variant.md).

authors: bruceg
