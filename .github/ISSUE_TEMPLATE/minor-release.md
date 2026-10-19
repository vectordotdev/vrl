---
name: VRL minor release
about: Use this template for a new minor release.
title: "VRL [version] release"
labels: "domain: releasing"
---

- [ ] (Optional) Preview the changelog: `cargo run -p release -- --dry-run`.
- [ ] Run the [Prepare release](https://github.com/vectordotdev/vrl/actions/workflows/release_prepare.yml) workflow on `main` (`version` defaults to `minor`; use `major`, `patch`, or an exact version to override). Put the link to this issue in the `issue` input. See [release/README.md](https://github.com/vectordotdev/vrl/blob/main/release/README.md) for details.
- [ ] Review, approve, and merge the `chore(releasing): prepare vX.Y.Z` PR opened by `vectordotdev-bot`.
- [ ] Confirm that the [Tag release](https://github.com/vectordotdev/vrl/actions/workflows/release_autotag.yml) workflow created the new tag: https://github.com/vectordotdev/vrl/tags
- [ ] Confirm that the [Publish release](https://github.com/vectordotdev/vrl/actions/workflows/release_publish.yml) workflow published the new VRL release: https://crates.io/crates/vrl
