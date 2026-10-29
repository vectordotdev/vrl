# VRL Release Tool

Prepares the files for a VRL release. The release workflows run it.

## How to release

1. Run [Prepare release](https://github.com/vectordotdev/vrl/actions/workflows/release_prepare.yml)
   on `main`. Set `version` to `minor` (default), `major`, `patch`, or an exact version such as
   `1.2.3`.
2. Review, approve, and merge the `chore(releasing): prepare vX.Y.Z` PR. To change the
   changelog, push edits to the PR branch.
3. After the merge, the `vX.Y.Z` tag is pushed and the crate is published to crates.io. Confirm
   the [tag](https://github.com/vectordotdev/vrl/tags) and the
   [crate](https://crates.io/crates/vrl).

If the merge queue rejects the release PR because of remaining changelog fragments, a different
PR merged after Prepare release ran. Close the release PR, delete its branch, and run Prepare
release again.

## Local usage

```shell
# Preview the changelog without changes
cargo run -p release -- --dry-run

# Bump the version and generate the changelog (minor by default)
cargo run -p release -- [major|minor|patch|X.Y.Z]

# Validate changelog fragment filenames
cargo run -p release -- check-changelog
```

The tool does not commit, tag, push, or publish.
