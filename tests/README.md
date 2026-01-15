# VRL Tests

Some tests are organized to mirror source code structure:
```
src/compiler/function.rs  →  tests/compiler/function/*/
src/parser/lex.rs  →  tests/parser/lex/*/
```

## Trybuild Tests (Compile-Time)

Trybuild tests verify code fails to compile with correct error messages. Defined in `tests/trybuild.rs`.

### Quick Workflow

1. **Create test file**: `tests/compiler/function/example/my_test.rs`
2. **Run tests**: `cargo test --test trybuild` (creates `wip/my_test.stderr`)
3. **Review error**: `cat wip/my_test.stderr`
4. **Accept (bless)**: `mv wip/my_test.stderr tests/compiler/function/example/`
5. **Add to suite**: Edit `tests/trybuild.rs`, add `t.compile_fail("tests/.../my_test.rs")`
6. **Commit both files**: `.rs` and `.stderr` files together

### Important: Commit .stderr Files

**Always commit `.stderr` files** - they define expected error messages and enable regression detection.

### Update Error Messages

```bash
TRYBUILD=overwrite cargo test --test trybuild
git diff tests/  # Review changes
```

## Commands

```bash
cargo test                                          # All tests
cargo test --test trybuild                          # Trybuild only
cargo test --test trybuild compiler_function_example # Specific test
```

Note: `trybuild` tests do not show up in code coverage reports.

## Reference

- [trybuild docs](https://docs.rs/trybuild/)
- [Rust testing guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
