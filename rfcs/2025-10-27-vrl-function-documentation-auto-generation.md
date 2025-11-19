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
- Generate intermediate JSON format for all VRL stdlib functions

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

4. **JSON output and distribution**

- VRL generates `vrl-stdlib-doc.json` and commits it to the repository
- JSON is the canonical output format from VRL
- Published as part of VRL releases for downstream consumers

5. **Vector's CUE transformation (Vector repository responsibility)**

- Vector consumes `vrl-stdlib-doc.json` from VRL releases
- Vector implements JSON→CUE transformation for its website needs
- Maintains existing CUE structure and formatting conventions
- Generates files in `website/cue/reference/remap/functions/`

6. **Testing integration**

- Ensure examples in documentation are validated by VRL test suite
- Prevent documentation examples from diverging from actual behavior

### Out of scope

1. **Website rendering changes** - This RFC focuses on generating JSON; website rendering remains unchanged
2. **Vector-specific functions** - Focus is VRL stdlib only (see Future work for Vector adoption)

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
┌─────────────────────────────────────────────────────────────────┐
│                     VRL Function Source Code                    │
│         (src/stdlib/ in VRL repository)                         │
│                                                                 │
│  - Function trait implementations                               │
│  - Structured documentation attributes/comments                 │
│  - Examples integrated with tests                               │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         │ (1) Parse & Extract
                         │
                         ▼
┌────────────────────────────────────────────────────────────────┐
│              Documentation Extraction Tool                     │
│             (Rust binary using syn/proc_macro)                 │
│                                                                │
│  - Parse Rust AST to find Function implementations             │
│  - Extract metadata from trait methods                         │
│  - Parse documentation attributes                              │
│  - Validate completeness                                       │
└────────────────────────┬───────────────────────────────────────┘
                         │
                         │ (2) Generate
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│           JSON Documentation Output                             │
│         (vrl-stdlib-doc.json)                                   │
│                                                                 │
│  {                                                              │
│    "functions": [                                               │
│      {                                                          │
│        "name": "get_secret",                                    │
│        "category": "System",                                    │
│        "description": "...",                                    │
│        "parameters": [...],                                     │
│        "examples": [...],                                       │
│        "return_type": [...]                                     │
│      }                                                          │
│    ]                                                            │
│  }                                                              │
│                                                                 │
│  - Committed to VRL repository                                  │
│  - Published with VRL releases                                  │
│  - Canonical documentation format                               │
└────────────────────────┬────────────────────────────────────────┘
                         │
                         │ (3) Consume (Vector repository)
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│        Vector's CUE Transformation                              │
│     (Vector fetches JSON and generates CUE)                     │
│                                                                 │
│  - Fetches vrl-stdlib-doc.json from VRL releases                │
│  - Transforms JSON to CUE format                                │
│  - Generates website/cue/reference/remap/functions/*.cue        │
│  - Maintains existing CUE structure                             │
└─────────────────────────────────────────────────────────────────┘
```

TODO

### Technical approach

TODO

## Future work

**Adoption by other projects:**

- Apply same generation process to Vector-specific functions in `lib/vector-vrl/functions/`
- VRL playground integration with live examples

**Format evolution:**

- **Publish formal JSON Schema**: Define versioned JSON Schema for `vrl-stdlib-doc.json` for validation and tooling support
- **Eliminate CUE transformation in Vector**: Long-term, migrate Vector's website to consume JSON directly, removing the JSON→CUE transformation entirely

**Additional capabilities:**

- Additional output formats (Markdown, HTML, OpenAPI-style) for other use cases
- IDE integration via LSP
- Localization/i18n support
- Documentation versioning across releases

## Success metrics

1. **Documentation coverage**: 100% of VRL stdlib functions have auto-generated documentation
2. **Documentation freshness**: JSON is always auto-generated from code (enforced by CI)
3. **Example accuracy**: 100% of examples pass when executed as tests
4. **Developer efficiency**: Time to document new function reduced by 50%+
5. **CI reliability**: Documentation checks catch drift in 100% of cases

## Alternatives Considered

### Alternative 1:

TODO more alternatives

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [Rust syn crate](https://docs.rs/syn/) - AST parsing
- [CUE Language](https://cuelang.org/) - Configuration language
