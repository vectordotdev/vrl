The Rust `Function` trait now uses a single `description()` method for function documentation. `usage()` has been renamed and `summary()` has been removed.

**Before:** callers used `function.usage()` or `function.summary()`.

**After:** callers use `function.description()`. Consumers that need a short preview can use its first paragraph. Custom `Function` implementations must rename `usage` to `description` and remove any `summary` override.

Existing description text and the generated JSON `description` field are preserved. VRL programs require no changes.

authors: pront
