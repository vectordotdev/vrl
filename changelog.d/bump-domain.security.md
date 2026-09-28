Bump `domain` from 0.12.2 to 0.12.3 to fix [RUSTSEC-2026-0310](https://rustsec.org/advisories/RUSTSEC-2026-0310.html), which covers panics, soundness issues, and excessive CPU or memory use when processing malicious DNS data.

`dns_lookup` now rejects names with more than 255 compression pointers. Newly recognized IANA record types use their assigned names in `recordType` and `questionType`; for example, `TYPE66` becomes `DSYNC`. Programs that compare these strings must use the new names or the unchanged `recordTypeId` and `questionTypeId` fields.

The Rust API changes in the upstream `unstable-zonetree` feature do not affect VRL, which does not enable that feature.

authors: thomasqueirozb
