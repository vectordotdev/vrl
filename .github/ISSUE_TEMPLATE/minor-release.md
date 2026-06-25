---
name: VRL minor release
about: Use this template for a new minor release.
title: "VRL [version] release"
labels: "domain: releasing"
---

- [ ] Run `cargo run -p release` (use `--dry-run` to preview, `--issue <url>` to link this issue)
  - [ ] Review/edit `CHANGELOG.md` when prompted, then press Enter to push and create the prep PR
  - [ ] Get the prep PR reviewed and merged (the script will poll and continue automatically)
  - The script will publish to crates.io, tag, and push the tag
- [ ] Confirm the new VRL release was published: https://crates.io/crates/vrl
- [ ] Confirm that the new tag was created: https://github.com/vectordotdev/vrl/tags
