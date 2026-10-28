# VRL Release Tool

A Rust binary that prepares the files for a VRL release. The GitHub workflows run it and do
the rest of the release.

## Release flow

1. A maintainer runs the [Prepare release](https://github.com/vectordotdev/vrl/actions/workflows/release_prepare.yml)
   workflow on `main`. The `version` input is `minor` (default), `major`, `patch`, or an exact
   version such as `1.2.3`. The optional `issue` input links a release issue in the PR.
2. The workflow runs this tool, and `vectordotdev-bot` opens the
   `chore(releasing): prepare vX.Y.Z` PR from the `prepare-X.Y.Z-release` branch.
3. A maintainer reviews, approves, and adds the PR to the merge queue. To change the changelog,
   push edits to the PR branch.
4. The merge queue rejects the release PR if a changelog fragment would remain in `changelog.d/`
   at the release commit. This happens when another PR merged after Prepare release ran, so the
   changelog misses its entry. In that case, close the release PR, delete its branch, and run
   Prepare release again. Other PRs can merge while the release PR is open.
5. After the PR merges into `main`, [Tag release](https://github.com/vectordotdev/vrl/actions/workflows/release_autotag.yml)
   finds the commit that raised the version in `Cargo.toml` and pushes the `vX.Y.Z` tag on it.
   The merge queue can merge other PRs in the same push, so this is not always the newest
   commit on `main`. Tag release makes the same fragment check, for merges that skip the queue.
   Confirm that the tag exists: https://github.com/vectordotdev/vrl/tags
6. The tag starts [Publish release](https://github.com/vectordotdev/vrl/actions/workflows/release_publish.yml),
   which publishes the crate to crates.io with
   [trusted publishing](https://crates.io/docs/trusted-publishing). No crates.io token is
   stored in the repository. Confirm that the release is published: https://crates.io/crates/vrl

## Usage

### Prepare (default)

```shell
# Bump minor version (default) and generate the changelog
cargo run -p release

# Bump a specific component
cargo run -p release -- major
cargo run -p release -- patch

# Exact version
cargo run -p release -- 1.2.3

# Dry run (preview the changelog without changes)
cargo run -p release -- --dry-run
```

The tool:
1. Resolves and validates the version (not already on crates.io)
2. Generates the changelog section from `changelog.d/` fragments, inferring contributor PRs from
   commit titles (this needs the full Git history)
3. Bumps the version in `Cargo.toml` and `Cargo.lock`
4. Adds the section to `CHANGELOG.md` and removes the fragments

It does not commit, tag, push, or publish.

### Workflow Subcommands

The release workflows call these subcommands. Values go to stdout and messages go to stderr.

```shell
# Print the version in Cargo.toml
cargo run -q -p release -- version

# Print the commit in <rev>..HEAD that raises the Cargo.toml version, as sha= and version=
# lines. Prints nothing if no commit raises it. Fails if the version goes down, if more than
# one commit raises it, or if changelog fragments remain at the release commit.
cargo run -q -p release -- release-commit --before <rev>

# Fail unless the tag is v<version> for the Cargo.toml version
cargo run -q -p release -- check-tag v1.2.3

# Print the release PR body. --issue is optional.
cargo run -q -p release -- pr-body --issue https://github.com/vectordotdev/vrl/issues/123
```

### Check Changelog Fragments

```shell
cargo run -p release -- check-changelog
```

Validates that changelog fragment filenames follow the `<description>.<type>.md` convention.
