---
name: VRL minor release
about: Use this template for a new minor release.
title: "VRL [version] release"
labels: "domain: releasing"
---

- [ ] (Optional) Preview the release: `cargo run -p release -- --dry-run`.
- [ ] Run the release: `cargo run -p release` (defaults to a minor bump; pass `major`, `patch`, or an exact version to override). This bumps `Cargo.toml`, generates the changelog, publishes to crates.io, tags, and opens the merge PR. See [release/README.md](https://github.com/vectordotdev/vrl/blob/main/release/README.md) for details.
- [ ] Review and merge the release PR opened by the tool.
- [ ] Confirm that the new tag was created: https://github.com/vectordotdev/vrl/tags
- [ ] Confirm that the new VRL release was published: https://crates.io/crates/vrl
