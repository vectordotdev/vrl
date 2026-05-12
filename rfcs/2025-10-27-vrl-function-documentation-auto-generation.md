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
  * Function documentation lives alongside/within its definition.
  * All examples are tested and validated before merging.
  * The corresponding JSON file is generated and included in the PR.
2. VRL is bumped in Vector and this commit is cherry-picked into the website.

## Goals

1. All 200+ VRL functions should maintain documentation quality at parity with or exceeding current manually-maintained CUE files.
2. All functions provide the fields needed for automatic documentation.
3. Vector and VRL each generate JSON documentation for their own functions, with no reliance on
   CUE files.
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

1. VRL repository
- Functions implement the `Function` trait.
- Functions are documented using required methods in the `Function` trait.
- All functions' examples are tested.
- One JSON file per stdlib function is generated and checked into the repo.
- CI checks ensure JSON files stay in sync with the source code.
- JSON generation code lives in VRL so Vector can consume it for Vector-specific functions and to facilitate reviews from the docs team.

2. Vector repository
- Vector-specific VRL functions also implement the `Function` trait.
- One JSON file per Vector-specific function is generated and checked into the repo, reusing the generation code from VRL.
- CI checks ensure JSON files stay in sync with the source code.
- CUE files for VRL functions are removed from the Vector repo.

3. Website (located in the Vector repo)
- Dynamically generates all functions' documentation using VRL's generation code when the website spins up, without needing to add VRL documentation in the Vector repo.

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

3. Provide documentation to the website. There are a couple of options here:
  * [Rejected] Directly convert documentation into json and insert it into `data/docs.json`
    - Pros
      * VRL source code is the single source of documentation and updating docs is simply running website
      deploy commands and one additional `vdev` command.
      * No binary documentation files or duplicated information in repos anywhere.
    - Cons
      * No documentation present in any CUE files in the Vector repo, making it harder to
      notice if the website needs to be updated.
      * Docs team and maintainers will probably not see any VRL documentation changes (during
        releases).
      * (minor) Less visibility into VRL documentation when checking out old source code
  * [Rejected] Convert documentation into CUE files and keep the regular flow.
    - Pros:
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.
    - Cons
      * VRL source code is not the single source of documentation.
      * VRL documentation has to be updated in two repos.
      * Need to generate CUE files when updating VRL.
      * We'd be generating CUE in a very hacky manner and we want to reduce our use of CUE.
  * **[Chosen]** Generate one JSON file per function in each repo.
    - Pros:
      * Docs team has visibility into documentation changes in both repos.
      * Single source of truth per function (no duplication across repos).
      * No CUE maintenance burden.
      * CUE files are removed from the Vector repo so source code is the single source of truth.
    - Cons
      * Need CI checks in both repos to catch differences in generated files.
      * The website dynamically generates all functions' documentation when it spins up. This will
        likely make the website deployment heavier and more complex than what it is right now.

3. Add JSON documentation genaration logic in VRL so that both repos can utilize it. The
   documentation will be dynamically generated based solely on the methods provided by
   the `Function` trait.

4. Add CI checks in both the VRL and Vector repos to ensure JSON files stay in sync with the
   source code, like what is done today with `check-component-docs`.

5. Create a `vdev` command (in Vector's repo) to generate docs and inject them into the website.


## Updating documentation before the next release

If any fixes or documentation updates need to be made before the next release, the following will
happen:

1. If this has not been done before during this release:
  * The `website-X.Y.Z` branch in VRL will be created from the current VRL version included in the repo,
    where `X.Y.Z` corresponds to that version.
  * The `website` branch in Vector will update its `Cargo.toml` to use VRL `branch = "website-X.Y.Z"`
    instead of `version = "X.Y.Z"`.
2. Commits updating documentation will be cherry-picked into the `website-X.Y.Z` branch in VRL.
3. In Vector's `website` branch, run `cargo update -p vrl`. Commit and push changes.

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
