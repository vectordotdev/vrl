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

## Proposal

### Solution

Implement an **automated documentation generation system** that:

1. **Extracts documentation from Rust source code** by extending the `Function` trait with documentation methods
2. **Auto-generates JSON file** (`vrl-stdlib-doc.json`) containing all function documentation - **no manual JSON writing**
3. **Publishes JSON as canonical output** - Vector and other consumers fetch JSON and handle their own transformations
4. **Integrates with test suite** to validate documentation examples

**Key workflow**: Developer implements documentation methods on `Function` trait → runs script → `vrl-stdlib-doc.json` is auto-generated → commit both code and JSON → Vector consumes JSON and transforms to CUE

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

### Documentation extraction approach: Extended Function trait

The proposed approach extends the existing `Function` trait with additional documentation methods. All function metadata lives in the trait implementation—no macros or attributes needed.

**Extended Function trait:**

```rust
pub trait Function: Send + Sync {
    // Existing methods (already implemented)
    fn identifier(&self) -> &'static str;
    fn parameters(&self) -> &'static [Parameter];
    fn examples(&self) -> &'static [Example];
    fn compile(/* ... */) -> Compiled;

    // NEW: Documentation methods (all with defaults for backwards compatibility)
    fn category(&self) -> &'static str {
        ""  // Default: empty (triggers warning in docs generation)
    }

    fn description(&self) -> &'static str {
        ""  // Default: empty (triggers warning in docs generation)
    }

    fn notices(&self) -> &'static [&'static str] {
        &[]  // Default: no notices
    }

    fn deprecated(&self) -> Option<&'static str> {
        None  // Default: not deprecated
    }
}
```

**Example implementation:**

```rust
#[derive(Clone, Copy, Debug)]
pub struct ParseJson;

impl Function for ParseJson {
    fn identifier(&self) -> &'static str {
        "parse_json"
    }

    fn category(&self) -> &'static str {
        "Parse"
    }

    fn description(&self) -> &'static str {
        "Parses the value as JSON."
    }

    fn notices(&self) -> &'static [&'static str] {
        &["Only JSON types are returned. Consider parse_timestamp for string→timestamp conversion."]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "max_depth",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "Parse JSON",
            source: r#"parse_json!("{\"key\": \"val\"}")"#,
            result: Ok(value!({key: "val"})),
        }]
    }

    fn compile(/* ... */) -> Compiled { /* ... */ }
}

impl FunctionExpression for ParseJsonFn {
    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::object(/* ... */)
            .fallible()
    }
}
```

**What gets extracted (all from trait methods):**

- ✅ Function name from `identifier()`
- ✅ Category from `category()`
- ✅ Description from `description()`
- ✅ Notices from `notices()`
- ✅ Deprecation from `deprecated()`
- ✅ Parameter names, types, required/optional from `parameters()`
- ✅ Example code and expected results from `examples()`
- ✅ Return types from `type_def()`
- ✅ Fallibility from `TypeDef::fallible()`/`infallible()`
- ✅ Purity from `TypeDef::impure()`

**Why this approach:**

1. **No macros needed**: Simple trait extension, standard Rust pattern
2. **Idiomatic**: Methods on traits are the standard way to define behavior
3. **Compiler enforced**: Missing `category()` or `description()` won't compile
4. **DRY principle**: No duplication—everything extracted from trait implementation
5. **Type safe**: All metadata in one place with compile-time checks
6. **Incremental improvement**: Can return empty strings initially, improve over time
7. **CI trackable**: Can measure progress: "45/150 functions documented"
8. **Validation**: Examples are already executable code in the codebase
9. **Discoverable**: IDE autocomplete shows all documentation methods

**Trade-off: Binary size**

Documentation strings are included in the final binary as static data. For ~150 functions with average 100 bytes of documentation each, this
adds ~15KB to binary size. This is acceptable because:

- VRL documentation is useful at runtime (error messages, introspection)
- 15KB is negligible compared to typical binary sizes (VRL library is several MB)
- Strings are in read-only data section (minimal memory impact)
- Alternative approaches (doc comments) require parsing source files, adding build complexity

### Intermediate JSON format

The tool **automatically generates** a JSON file (`vrl-stdlib-doc.json`) containing documentation for all functions. This intermediate
format is consumable by multiple systems (Vector docs, VRL playground, Observability Pipelines, etc.).

**Example output** (abbreviated):

```json
{
  "schema_version": "1.0.0",
  "generated_at": "2025-10-27T10:00:00Z",
  "functions": [
    {
      "name": "parse_json",
      "category": "Parse",
      "description": "Parses the value as JSON.",
      "arguments": [
        {
          "name": "value",
          "description": "The string representation of the JSON to parse.",
          "type": [
            "string"
          ],
          "required": true,
          "default": null
        },
        {
          "name": "max_depth",
          "description": "Number of layers to parse.",
          "type": [
            "integer"
          ],
          "required": false,
          "default": 128
        }
      ],
      "return_type": {
        "types": [
          "object",
          "array",
          "string",
          "integer",
          "float",
          "boolean",
          "null"
        ]
      },
      "internal_failure_reasons": [
        "value is not valid JSON"
      ],
      "examples": [
        {
          "title": "Parse JSON",
          "source": "parse_json!(\"{\\\"key\\\": \\\"val\\\"}\")",
          "result": {
            "type": "value",
            "value": {
              "key": "val"
            }
          }
        }
      ],
      "is_fallible": true,
      "is_pure": true,
      "deprecated": false
    }
  ]
}
```

**Type mapping**: VRL `Kind` values map to JSON types: `kind::BYTES` → `"string"`, `kind::INTEGER` → `"integer"`, `kind::FLOAT` → `"float"`,
etc. Complex types from `TypeDef` are resolved to their constituent types.

**Result serialization**: Example results (`Ok(value!({key: "val"}))`) are serialized using VRL's native Value serialization to
JSON-compatible structures.

### Implementation phases

#### Phase 1: Core extraction tool

**Deliverables**:

- Rust binary tool that parses VRL function source files
- AST parsing using `syn` crate to find `impl Function` blocks
- Extraction of metadata from trait methods:
  - Function name from `identifier()`
  - Category from `category()`
  - Description from `description()`
  - Parameters from `parameters()`
  - Examples from `examples()`
  - Notices from `notices()`
  - Deprecation from `deprecated()`
  - Return type from `type_def()` in FunctionExpression trait
- Output intermediate JSON format (trivial with `serde_json`)

**Testing**:

- Unit tests for AST parsing
- Integration tests with sample functions
- Validation that JSON schema is well-formed

#### Phase 2: Function trait extension and migration

**Deliverables**:

- Extend `Function` trait with new documentation methods (all with default implementations):
  - Add `category()` and `description()` returning `""` by default
  - Add `notices()` and `deprecated()` with appropriate defaults
- **No changes needed to existing functions** - they all compile with defaults
- Allows incremental improvement over time
- Validation and linting:
  - Warn on empty `category()` or `description()`
  - Error on invalid category values (must be from known set)
  - Check example completeness
  - Track documentation coverage metrics

#### Phase 3: Vector's CUE transformation (Vector repository)

**Deliverables** (implemented in Vector repository):

- CUE file generator that transforms JSON to CUE format
- Template system for CUE file structure
- Proper formatting and indentation
- Support for all CUE documentation features:
  - Multi-line descriptions (using CUE's `#"""` syntax)
  - Nested example structures
  - Enum value documentation
  - Cross-references and URLs
- Integration with Vector build process to fetch VRL's JSON

**Testing**:

- Compare generated CUE against existing manual CUE files
- Validate CUE syntax correctness
- Ensure website rendering works correctly

**Note on complexity and ownership**: CUE generation is complex as Rust lacks standard CUE serialization libraries, requiring custom templating, manual string escaping (CUE's `#"""` multi-line syntax, special characters), and careful formatting. By placing this responsibility in Vector (the CUE consumer), we keep VRL simple—it only outputs JSON using `serde_json`. Vector owns the CUE transformation complexity since it's Vector's website that requires CUE format. Long-term, Vector can migrate to consume JSON directly, eliminating this transformation entirely.

#### Phase 4: Test integration

**Deliverables**:

- Mechanism to run examples from documentation as tests
- Integration with VRL test suite
- CI/CD checks:
  - Verify all functions have documentation
  - Ensure documentation is up-to-date (fail if manual edits detected)
  - Validate examples execute correctly
- Migration guide for existing functions

**Testing**:

- Run full test suite with example validation
- Performance benchmarks
- Documentation coverage reports

#### Phase 5: Documentation improvement and rollout

**Deliverables**:

- Incrementally improve documentation for all VRL stdlib functions (~150 functions)
- Generate and commit JSON documentation continuously
- Update contributor documentation in VRL repo
- CI/CD integration for automated validation
- Deprecation of manual CUE file editing in Vector

**Migration strategy (incremental improvement)**:

**Initial rollout:**
1. Extend `Function` trait with new methods (all have default implementations returning empty strings)
2. Entire codebase compiles immediately without any changes to existing functions
3. Documentation tool extracts whatever is available (empty strings generate warnings)
4. CI runs in warn-only mode for empty documentation

**Incremental improvement:**
5. Validate against existing CUE files to populate initial values
6. Contributors improve documentation in small PRs (5-10 functions at a time)
7. CI tracks coverage: "45/150 functions fully documented (30%)"
8. Review process catches missing/incomplete docs

**Enforcement:**
9. Once >90% coverage achieved, CI switches to error mode for empty docs
10. New functions must include documentation to pass CI

**Benefits of this approach:**
- No big-bang migration required
- Codebase always compiles and generates docs (even if incomplete)
- Clear progress tracking
- Encourages iterative improvement
- Lower barrier for contribution

### Implementation details

#### Tool structure

New crate `crates/vrl-doc/` in VRL repository with modules:

- `extractor/`: AST parsing with `syn`, trait method analysis and extraction
- `schema/`: JSON schema definitions and serialization
- `validator/`: Documentation completeness validation and linting

Note: CUE generation will be implemented in Vector repository as it's the consumer of CUE format. No proc macros needed.

#### CLI interface

```bash
# Generate JSON from VRL stdlib
cargo run --bin vrl-doc extract --input src/stdlib --output vrl-stdlib-doc.json

# Validate completeness
cargo run --bin vrl-doc validate --stdlib-dir src/stdlib
```

#### CI/CD integration

**VRL repository**:
- CI validates docs are up-to-date and complete
- Commits `vrl-stdlib-doc.json` to repository
- Publishes JSON as part of releases

**Vector repository**:
- Fetches `vrl-stdlib-doc.json` from VRL releases/artifacts
- Runs JSON→CUE transformation during build
- Generates CUE files in `website/cue/reference/remap/functions/`

**Synchronization**: Vector updates docs when bumping VRL dependency version

## Rationale

### Why automated generation?

1. **Single repository workflow**: Contributors only need to work in VRL repo - no cross-repository PRs required
2. **Single source of truth**: Documentation lives with code in the same repository, reducing drift
3. **Eliminates coordination overhead**: No need to sync and merge PRs across VRL and Vector repos
4. **Lower contribution barrier**: External contributors don't need Vector repo access or familiarity
5. **Consistency**: All functions documented with same structure and quality
6. **Validation**: Examples can be tested in VRL's test suite, ensuring accuracy
7. **Reduced maintenance**: Updates happen automatically with code changes
8. **Better DX**: Developers only update one place (VRL Rust code)

### Why JSON as canonical output format?

1. **Flexibility**: Multiple consumers can use the same data (Vector docs, VRL playground, Observability Pipelines)
2. **Simplicity**: JSON generation is trivial with `serde_json`; no custom templating or escaping required
3. **Separation of concerns**: Each consumer handles their own format transformations (Vector→CUE, playground→UI, etc.)
4. **Stability**: JSON schema provides versioned contract between VRL and consumers
5. **Debugging**: JSON can be inspected and validated independently
6. **Extensibility**: New output formats can consume JSON without changing VRL code

### Why start in VRL repo?

This RFC focuses on VRL stdlib functions first because:

1. **Core problem location**: The GitHub issue was filed in the VRL repo, identifying this as the primary pain point
2. **Broader impact**: VRL stdlib functions are used by Vector, VRL playground, Observability Pipelines, and other consumers
3. **Single source of truth**: All ~150+ core VRL functions are defined in one repository
4. **Largest benefit**: The bulk of documentation maintenance burden comes from stdlib functions
5. **Natural ownership**: VRL repo owns both the function implementations and their documentation

## Drawbacks

1. **Build complexity**: Adds another build step and tool to maintain
2. **Documentation burden**: Contributors must implement documentation methods for new functions
3. **Validation complexity**: Need tooling to validate categories, track coverage, etc.
4. **Coordination with Vector**: Vector must implement JSON→CUE transformation
5. **Quality control**: Default empty strings allow functions with no documentation (mitigated by CI warnings/errors)

## Prior art

Similar documentation generation systems:

1. **Rust's `rustdoc`** - Extracts docs from code, proves viability of code-as-source-of-truth
2. **Vector's config schema generation** - Already generates JSON schema from Rust using AST parsing
3. **OpenAPI/Swagger** - Shows value of intermediate format for multiple consumers
4. **Python Sphinx, JSDoc** - Documentation extracted from code annotations

## Alternatives

### Alternative 1: Attribute macro annotations

**Description**: Use attribute macros to specify documentation metadata on structs:

```rust
#[derive(Clone, Copy, Debug)]
#[vrl_doc(
    category = "Parse",
    description = "Parses the value as JSON.",
)]
pub struct ParseJson;

impl Function for ParseJson {
    fn identifier(&self) -> &'static str { "parse_json" }
    fn parameters(&self) -> &'static [Parameter] { /* ... */ }
    // ...
}
```

**Pros**:
- Compact syntax
- Visual separation of documentation from implementation
- Flexible metadata fields

**Cons**:
- Proc macro complexity (hard to debug)
- Learning curve for new syntax
- Big-bang migration (all functions need macro before it works)
- No compile-time enforcement (forgot macro? Silent failure)
- Requires additional macro crate
- Cannot have default implementations (attributes are all-or-nothing)

### Alternative 2: Structured doc comments

**Description**: Use structured Rust doc comments with specific format:

```rust
/// Parses the value as JSON.
///
/// # Category
/// Parse
///
/// # Parameters
/// - `value` (string, required): The string representation of the JSON to parse.
/// - `max_depth` (integer, optional): Number of layers to parse for nested JSON.
///
/// # Returns
/// - object: The parsed JSON object
///
/// # Examples
/// ```vrl
/// parse_json!("{\"key\": \"val\"}")
/// # => {key: "val"}
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ParseJson;
```

**Pros**:

- Uses standard Rust doc comments
- Works with existing Rust tooling (`rustdoc`)
- Familiar to Rust developers
- **Zero runtime cost**: Doc comments are compiled out, no strings in final binary
- **Lower binary size**: Documentation doesn't contribute to executable size

**Cons**:

- **Still requires learning custom format**: VRL-specific sections (`# Category`, `# Parameters`, etc.) not standard Rust
- Free-form text harder to parse reliably
- No compile-time validation of structure
- Requires complex parsing logic with fragile regex/markdown parsing
- Easy to have inconsistent formatting (typos like `# Catagory` silently ignored)
- Duplicates type information (comments vs. code)
- No out-of-the-box IDE support for custom doc comment structure
- Must parse source files at build time (can't extract from compiled code)

### Alternative 3: Reverse generation (CUE → Rust)

Keep CUE as source of truth, generate Rust validation. **Rejected** - Rust is actual source of truth for behavior; CUE can't capture type
inference and compile-time semantics.

### Alternative 4: Use `rustdoc` with custom output

Use standard `rustdoc` tooling. **Rejected** - `rustdoc` targets Rust API docs, not VRL user-facing docs; can't generate JSON/CUE or capture
VRL-specific metadata.

## Key questions addressed

**Vector-specific functions**: Out of scope for this RFC; see Future work for Vector adoption path.

**Stale documentation**: CI checks fail if generated docs differ from committed version, forcing updates.

**Schema versioning**: `schema_version` field in JSON allows evolution; tooling supports multiple versions.

**External state in examples**: Use `input`/`output` sections for stateful examples (enrichment tables, etc.).

**Security**: Trait methods return static strings; no code execution risk; standard code review applies.

## Feasibility

This proposal is technically feasible using well-established Rust tooling and patterns.

### Technical approach

**AST parsing with `syn`**

Rust's `syn` crate provides robust AST parsing capabilities used by many production tools (`serde_derive`, `cargo-doc`, etc.). We can parse
VRL function source files to extract metadata:

```rust
// Parse Rust source file
let syntax_tree: syn::File = syn::parse_file( & source_code) ?;

// Find impl Function blocks
for item in syntax_tree.items {
if let syn::Item::Impl(impl_block) = item {
if implements_function_trait( & impl_block) {
// Extract from trait methods
extract_identifier( & impl_block);    // fn identifier() -> &str
extract_parameters( & impl_block);    // fn parameters() -> &[Parameter]
extract_examples( & impl_block);      // fn examples() -> &[Example]
}
}
}
```

This is the same pattern used by derive macros throughout the Rust ecosystem.

**All information extracted from trait methods**

All documentation metadata is extracted from `Function` trait method implementations:

- Function name from `identifier()` method
- Category from `category()` method
- Description from `description()` method
- Notices from `notices()` method
- Deprecation from `deprecated()` method
- Parameter types from `parameters()` array
- Examples from `examples()` array
- Return types from `type_def()` method

Everything lives in the trait implementation—single source of truth, no duplication.

**JSON as intermediate format**

Generating JSON provides:

- **Decoupling**: VRL generates docs, Vector/others consume independently
- **Flexibility**: Multiple consumers (Vector docs, VRL playground, Observability Pipelines)
- **Versioning**: Schema version field allows evolution
- **Validation**: Existing manual CUE docs provide validation baseline

### Proven approach

The technical approach is proven by existing systems:

- **Vector's config schema generation** uses AST parsing for JSON schema
- **`serde_derive`**, **`cargo-doc`** use `syn` for similar extraction
- These demonstrate the approach is production-ready

### Migration de-risking

The ~150 existing CUE files provide a **validation baseline**:

1. Extend `Function` trait with new methods (all have defaults)
2. Implement documentation methods for functions incrementally
3. Generate docs and compare to existing manual CUE docs
4. Target: >90% exact matches validates extraction logic
5. Discrepancies reveal either bugs or documentation improvements

### Implementation confidence

- **Complex return types**: Extract from `type_def()` which encodes full type info
- **Performance**: AST parsing with `syn` is fast; <1 second for 150+ functions
- **Versioning**: JSON schema includes version field for evolution

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

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [Rust syn crate](https://docs.rs/syn/) - AST parsing
- [CUE Language](https://cuelang.org/) - Configuration language
