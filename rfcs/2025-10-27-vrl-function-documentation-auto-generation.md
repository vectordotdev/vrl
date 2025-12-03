# RFC 2025-10-27 - VRL Function Documentation Auto-Generation

## Context

VRL function documentation has always been manually maintained in separate CUE files. The separation of VRL into its own repository has made
the maintenance burden even **worse**: contributors adding VRL functions must now create PRs in **two repositories** - one for the function implementation (VRL
repo) and another for the documentation (Vector repo). This convoluted process leads to documentation
drift/undocumented functions, and incorrect examples (examples are not tested in the Vector repo, but are in VRL).

This RFC proposes an automated system for generating VRL function documentation directly from the Rust source code where functions are
defined, ensuring documentation stays synchronized with implementation and eliminating the need for cross-repository documentation PRs.

## Goals

1. All 200+ VRL functions should maintain documentation quality at parity with or exceeding current manually-maintained CUE files
2. 100% of VRL functions have the necessary attributes to automatically generate documentation
3. Vector automatically generates documentation for all VRL functions without relying on **any**
   non-Rust source code files.
4. Validate 100% of examples through CI-enforced testing
5. Require only a single PR to add new VRL functions

## Out of scope

1. **Website rendering changes** - This RFC focuses on documentation generation; website rendering should remain mostly unchanged
2. **Automatic VRL function discovery and/or AST parsing** - This is a nice project but currently
   all VRL functions are present in the `vrl::stdlib::all()` vector. This RFC focuses on
   documentation and not (code generation for) automatic function discovery

## Proposal

### Proposed brief architecture overview

1. VRL functions source code (src/stdlib in the VRL repository)
- Functions implement the `Function` trait.
- Functions are documented using required methods in the `Function` trait.
- All functions' examples are tested.

2. Vector Repo (consumes VRL)
- Aggregates VRL functions and internal VRL functions.
- Grabs all function information using the `Function` trait.
- Does necessary transforms/validations and generates output.
- Output is visible on the website.

### Technical approach

1. Expand the `Function` trait. Currently it contains `identifier`, `summary`, `usage`, `examples`. It is
   missing `internal_failure_reasons`, `description`, `return` and `category`. Note that `usage` and
   `description` should be equivalent.

2. Once the `Function` trait is updated, port all documentation currently present in Vector cue functions
   [here](https://github.com/vectordotdev/vector/blob/master/website/cue/reference/remap/functions/)
   into VRL's source code. Once the PR is merged, update VRL inside of Vector and do the same for
   Vector-specific VRL functions.
  * Note that after the `Function` trait is updated VRL will not be able to be updated in the Vector
    repo until all functions are documented.

3. Create a `vdev` command (in Vector's repo) to generate documentation based solely on the methods
   provided by the `Function` trait.

4. Provide documentation to the website. There are a couple of options here:
  * [Rejected] Directly convert documentation into json and insert it into `data/docs.json`
    - Cons
      * No documentation present in any cue files in the Vector repo, making it harder to
      notice if the website needs to be updated.
      * Docs team and maintainers will probably not see any VRL documentation changes (during
        releases).
      * (minor) Less visibility into VRL documentation when checking out old source code
    - Pros
      * VRL source code is the sole source of truth and updating docs is simply running website
      deploy commands and one additional `vdev` command.
      * No binary documentation files or duplicated information in repos anywhere.
  * [Rejected] Convert documentation into cue files and keep the regular flow.
    - Cons
      * VRL source code is not the sole source of truth.
      * VRL documentation has to be updated in two repos.
      * Need to generate cue files when updating VRL.
      * We'd be generating cue in a very hacky manner and we want to move away from cue wherever
        possible
    - Pros:
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.
  * Convert documentation into pretty printed JSON file.
    - Cons
      * VRL source code is not the sole source of truth.
      * VRL documentation has to be updated in two repos.
      * Need to generate json files when updating VRL.
    - Pros:
      * While VRL source code is not the sole source of truth, the docs can and will be generated
        from VRL directly.
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.

5. Add an extra step to `make generate-component-docs` to also automatically generate VRL function
   documentation and have CI fail if there are any unexpected changes to VRL generated documentation,
   making it so no PRs go in without properly updating the function documentation.

## Future work

- Make it so Vector-specific functions also have their examples tested with the same rigor as VRL
  examples. This could turn out to be a hard problem since some of them need Vector to be running
  and configured properly.

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [VRL stdlib](https://github.com/vectordotdev/vrl/tree/main/src/stdlib)
