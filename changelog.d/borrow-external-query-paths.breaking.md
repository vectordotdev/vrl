`Query::external_path` now returns `Option<&OwnedTargetPath>` to let callers borrow
the stored path. Rust callers that need an owned path must clone it explicitly.
VRL programs do not need to change.

**Before:**

```rust
let path: Option<OwnedTargetPath> = query.external_path();
```

**After:**

```rust
let path: Option<OwnedTargetPath> = query.external_path().cloned();
```

authors: gwenaskell
