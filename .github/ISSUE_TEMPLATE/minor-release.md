---
name: VRL minor release
about: Use this template for a new minor release.
title: "VRL [version] release"
labels: "domain: releasing"
---

- [ ] Create release preparation PR
  - [ ] Bump [Cargo.toml](https://github.com/vectordotdev/vrl/blob/main/Cargo.toml#L3) version and commit the change.
  - [ ] Run `cargo check` to update `Cargo.lock`.
  - [ ] Run the [./scripts/generate_release_changelog.sh](https://github.com/vectordotdev/vrl/blob/main/scripts/generate_release_changelog.sh) script
    and commit the changes.
- [ ] After the above PR is merged, run the [./scripts/publish.py](https://github.com/vectordotdev/vrl/blob/main/scripts/publish.py) script.
  - [ ] Confirm that the new tag was created: https://github.com/vectordotdev/vrl/tags
  - [ ] Confirm that the new VRL release was published: https://crates.io/crates/vrl
