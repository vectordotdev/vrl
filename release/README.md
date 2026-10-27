# VRL Release Tool

A Rust binary that prepares the files for a VRL release. The GitHub workflows run it and do
the rest of the release.

## Release flow

1. A maintainer runs the [Prepare release](https://github.com/vectordotdev/vrl/actions/workflows/release_prepare.yml)
   workflow on `main`. The `version` input is `minor` (default), `major`, `patch`, or an exact
   version such as `1.2.3`. The optional `issue` input links a release issue in the PR.
2. The workflow runs this tool, and `vectordotdev-bot` opens the
   `chore(releasing): prepare vX.Y.Z` PR from the `prepare-X.Y.Z-release` branch.
3. A maintainer reviews, approves, and merges the PR. To change the changelog, push edits to the
   PR branch.
4. After the PR merges into `main`, [Tag release](https://github.com/vectordotdev/vrl/actions/workflows/release_autotag.yml)
   sees the version change in `Cargo.toml` and pushes the `vX.Y.Z` tag on the merge commit.
   Confirm that the tag exists: https://github.com/vectordotdev/vrl/tags
5. The tag starts [Publish release](https://github.com/vectordotdev/vrl/actions/workflows/release_publish.yml),
   which publishes the crate to crates.io with
   [trusted publishing](https://crates.io/docs/trusted-publishing). No crates.io token is
   stored in the repository. Confirm that the release is published: https://crates.io/crates/vrl

> [!IMPORTANT]
> Do not merge other PRs while the release PR is open. The changelog only includes the fragments
> that were on `main` when the Prepare release workflow ran. If another PR merges, close the
> release PR, delete its branch, and run Prepare release again.

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

# Print the version to tag if the Cargo.toml version went up since <rev>.
# Prints nothing if the version did not change. Fails if it went down.
cargo run -q -p release -- bumped-version --before <rev>

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
