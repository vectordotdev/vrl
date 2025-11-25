# RFC 2025-10-27 - VRL Function Documentation Auto-Generation

## Context

VRL function documentation has always been manually maintained in separate CUE files (`website/cue/reference/remap/functions/*.cue` in the
Vector repository), which is error-prone and leads to documentation drift. The separation of VRL into its own repository has made this
problem **worse**: contributors adding VRL functions must now create PRs in **two repositories** - one for the function implementation (VRL
repo) and another for the documentation (Vector repo). This cross-repository requirement creates friction, increases the likelihood of
missing or outdated documentation, and makes the contribution process more complex.

This RFC proposes an automated system for generating VRL function documentation directly from the Rust source code where functions are
defined, ensuring documentation stays synchronized with implementation and eliminating the need for cross-repository documentation PRs.

## Cross references

- **GitHub Issue**: [vectordotdev/vrl#280](https://github.com/vectordotdev/vrl/issues/280) - Original feature request
- **Related Vector Docs
  **: [website/cue/reference/remap/functions/](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions) -
  Current manual documentation location
- **VRL Stdlib**: Each own dedicated repo (not Vector) `https://github.com/vectordotdev/vrl.git`

## Scope

### In scope

1. **Documentation extraction from Rust source code**

- Extract function metadata from VRL `Function` trait implementations
- Parse structured documentation comments/annotations

2. **VRL stdlib function documentation**

- Document all functions in VRL stdlib (`src/stdlib/` in this repository)
- ~150+ core VRL functions (parse_json, parse_syslog, encode_base64, etc.)
- Functions used across all VRL implementations (Vector, standalone, etc.)

3. **Documentation schema design**

- Function signature (name, parameters, return types)
- Parameter details (name, type, required/optional, defaults, enums)
- Examples with expected output
- Error conditions (internal_failure_reasons)
- Category classification
- Additional metadata (notices, deprecation, etc.)

4. **Vector's documentation of VRL (Vector repository responsibility)**

- Vector aggregates functions from `vrl::stdlib::all()` from the current VRL version
- Vector also aggregates its own internal functions
- Documentation is automatically generated
- Website shows documentation

5. **Testing integration**

- Ensure examples in documentation are validated by VRL test suite
- Prevent documentation examples from diverging from actual behavior

### Out of scope

1. **Website rendering changes** - This RFC focuses on generating JSON; website rendering remains unchanged
2. **Vector-specific functions** - Focus is VRL stdlib only (see Future work for Vector adoption)
3. **Automatic VRL function discorery and/or AST parsing** - This is a nice project but currently
   all VRL functions are present in the `vrl::stdlib::all()` vector. This RFC focuses on
   documentation and not (code generation for) automatic function discovery

## Pain points

### Current state problems

Adding a VRL stdlib function requires **two PRs in separate repositories**:

1. **VRL repo**: Implement function in Rust with parameters, examples, and tests
2. **Vector repo**: Manually create matching CUE documentation file with same information

**Critical issues**:

- **Cross-repository coordination**: PRs must be kept in sync and merged in order
- **Access barriers**: External contributors need permissions/familiarity with both repos
- **Documentation drift**: Code changes in VRL don't trigger docs updates in Vector
- **Missing documentation**: Functions may be merged without corresponding docs
- **Duplicate maintenance**: Same info (types, examples) written twice in different formats
- **Validation gap**: Examples in docs aren't validated by VRL's test suite
- **Review burden**: Coordination overhead across two repositories

## Prior art

Similar documentation generation systems:

1. **Rust's `rustdoc`** - Extracts docs from code, proves viability of code-as-source-of-truth
2. **Vector's config schema generation** - Already generates JSON schema from Rust using AST parsing
3. **OpenAPI/Swagger** - Shows value of intermediate format for multiple consumers
4. **Python Sphinx, JSDoc** - Documentation extracted from code annotations

## Proposal

### Architecture overview

```
         ┌───────────────────────────────────────────────────┐
         │            VRL Function Source Code               │
         │         (src/stdlib/ in VRL repository)           │
         │                                                   │
         │  - Function trait implementations                 │
         │  - Structured documentation attributes/comments   │
         │  - Examples integrated with tests                 │
         └────────────────────────┬──────────────────────────┘
                                  │
                                  │ Consume (Vector repository)
                                  │
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   Vector's CUE Transformation                       │
│                      (Vector generates CUE)                         │
│                                                                     │
│  - Uses vrl's stdlib functions and internal VRL functions           │
│  - Transform function information and make available in the website │
└─────────────────────────────────────────────────────────────────────┘
```

### Technical approach

1. Expand the `Function` trait. Currently it contains `identifier`, `summary`, `usage`, `examples`. It is
   missing `internal_failure_reasons`, `description`, `return` and `category`. Note that `usage` and
   `description` should be equivalent.

2. Once the `Function` trait is updated, port all documentation currently present in Vector cue functions
   [here](https://github.com/vectordotdev/vector/blob/master/website/cue/reference/remap/functions/)
   into VRL's source code. Once the PR is merged, update VRL inside of Vector and do the same for
   Vector-specific VRL functions.

3. Create a vdev command (in Vector's repo) to generate documentation based solely on the methods
   provided by the `Function` trait.

4. Provide documentation to the website. There are couple of options here:
  * [Rejected] Directly convert documentation into json and insert it into `data/docs.json`
    - Cons
      * no documentation present in any cue files in the Vector repo, making it harder to
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
      * More visibility into documentation changes. This makes it easier to notice if the website needs
      to be updated since CI checks will catch differences in generated files.

5. Add an extra step to `make generate-component-docs` to also automatically generate VRL examples
   and have CI fail if there are any unexpected changes to VRL generated documentation.

## Future work

**Additional capabilities:**

- Generate additional output formats (Markdown, HTML, OpenAPI-style) for other use cases
- IDE integration via LSP
- Localization/i18n support

## Success metrics

1. **Documentation coverage**: 100% of VRL stdlib functions have auto-generated documentation
2. **Documentation freshness**: CI enforced documentation generation
5. **CI reliability**: Documentation checks catch drift in 100% of cases
3. **Example accuracy**: 100% of examples pass when executed as tests
4. **Developer efficiency**: New VRL functions only need 1 PR

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [Rust syn crate](https://docs.rs/syn/) - AST parsing
- [CUE Language](https://cuelang.org/) - Configuration language
