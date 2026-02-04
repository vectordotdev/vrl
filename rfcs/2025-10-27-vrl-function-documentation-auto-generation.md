# RFC 2025-10-27 - VRL Function Documentation Auto-Generation

## Context

VRL function documentation lives in two repos today, must be updated by hand, and easily goes stale.
This RFC proposes moving all VRL function documentation into function definition source code, so
Vector can generate documentation automatically and all examples are validated in CI.

Today contributing a new VRL function requires creating PRs in two separate repositories:
1. VRL repository - Function implementation.
2. Vector repository - Adds documentation in a _manually_ created/maintained CUE file.

This split workflow creates several problems:
- Documentation becomes outdated or incorrect.
- Examples in CUE files are not tested, leading to broken examples.
- Functions can ship without documentation if the Vector PR is missed.
- Contributors must open two PRs for one change.

Once this RFC is implemented, the workflow of adding and documenting a new VRL function should look like this:
1. PR is opened in the VRL repository.
  * Function documentation lives alongside/within it's definition.
  * All examples are tested and validated before merging.
2. VRL function documentation is automatically updated in the Vector repo once VRL is bumped.

## Goals

1. All 200+ VRL functions should maintain documentation quality at parity with or exceeding current manually-maintained CUE files.
2. All functions provide the fields needed for automatic documentation.
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

We will move all VRL function docs into the `Function` trait, port existing docs into VRL,
and add a `vdev` command that generates website-ready docs from VRL functions alone.

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

2. Once the `Function` trait is updated, port all documentation currently present in Vector CUE functions
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
      * No documentation present in any CUE files in the Vector repo, making it harder to
      notice if the website needs to be updated.
      * Docs team and maintainers will probably not see any VRL documentation changes (during
        releases).
      * (minor) Less visibility into VRL documentation when checking out old source code
    - Pros
      * VRL source code is the single source of documentation and updating docs is simply running website
      deploy commands and one additional `vdev` command.
      * No binary documentation files or duplicated information in repos anywhere.
  * [Rejected] Convert documentation into CUE files and keep the regular flow.
    - Cons
      * VRL source code is not the single source of documentation.
      * VRL documentation has to be updated in two repos.
      * Need to generate CUE files when updating VRL.
      * We'd be generating CUE in a very hacky manner and we want to reduce our use of CUE.
    - Pros:
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.
  * **[Chosen]** Convert documentation into pretty printed JSON file.
    - Cons
      * VRL source code is not the single source of documentation.
      * VRL documentation has to be updated in two repos.
      * Need to generate json files when updating VRL.
    - Pros:
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.

5. Add an extra step to `make generate-component-docs` to also automatically generate VRL function
   documentation and have CI fail if there are any unexpected changes to VRL generated documentation,
   making it so no PRs go in without properly updating the function documentation.

## Future work

- Make it so Vector-specific functions also have their examples tested with the same rigor as VRL
  examples. Vector-only functions usually require a running Vector instance, so testing their
  examples will require a new test harness.

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [VRL stdlib](https://github.com/vectordotdev/vrl/tree/main/src/stdlib)
