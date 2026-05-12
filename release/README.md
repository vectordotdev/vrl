# VRL Release Tool

A Rust binary that handles the full VRL release process.

## Usage

### Release (default)

```shell
# Bump minor version (default), generate changelog, publish, tag, create PR
cargo run -p release

# Bump a specific component
cargo run -p release -- major
cargo run -p release -- patch

# Exact version
cargo run -p release -- 1.2.3

# Dry run (preview without changes)
cargo run -p release -- --dry-run

# Link a GitHub issue
cargo run -p release -- --issue https://github.com/vectordotdev/vrl/issues/123
```

The release flow:
1. Resolves and validates the version (not already on crates.io)
2. Creates a release branch
3. Bumps version in `Cargo.toml`
4. Generates changelog from `changelog.d/` fragments
5. Pauses for you to review/edit `CHANGELOG.md`
6. Publishes to crates.io
7. Tags and pushes
8. Creates a PR to merge the release into main

### Check Changelog Fragments

```shell
cargo run -p release -- check-changelog
```

Validates that changelog fragment filenames follow the `<pr_number>.<type>.md` convention.
