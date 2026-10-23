The Rust types exposed by `vrl::protobuf` now use `prost` 0.14 and `prost-reflect` 0.16. Rust integrations that pass these types to or from VRL must use compatible dependency versions.

**Before:**

```toml
prost = "0.13"
prost-reflect = "0.14"
```

**After:**

```toml
prost = "0.14"
prost-reflect = "0.16"
```

No changes to VRL programs are required. The protobuf wire format is unchanged.

authors: thomasqueirozb
