# RFC 2025-10-27 - VRL Function Documentation Auto-Generation

## Context

VRL function documentation has always been manually maintained in separate CUE files (`website/cue/reference/remap/functions/*.cue` in the Vector repository), which is error-prone and leads to documentation drift. The separation of VRL into its own repository has made this problem **worse**: contributors adding VRL functions must now create PRs in **two repositories** - one for the function implementation (VRL repo) and another for the documentation (Vector repo). This cross-repository requirement creates friction, increases the likelihood of missing or outdated documentation, and makes the contribution process more complex.

This RFC proposes an automated system for generating VRL function documentation directly from the Rust source code where functions are defined, ensuring documentation stays synchronized with implementation and eliminating the need for cross-repository documentation PRs.

## Cross references

- **GitHub Issue**: [vectordotdev/vrl#280](https://github.com/vectordotdev/vrl/issues/280) - Original feature request
- **Related Vector Docs**: [website/cue/reference/remap/functions/](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions) - Current manual documentation location
- **VRL Stdlib**: External dependency at `https://github.com/vectordotdev/vrl.git`

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

4. **CUE file generation**
   - Transform JSON intermediate format to CUE documentation files
   - Maintain existing CUE structure and formatting conventions
   - Generate files in `website/cue/reference/remap/functions/`

5. **Testing integration**
   - Ensure examples in documentation are validated by VRL test suite
   - Prevent documentation examples from diverging from actual behavior

### Out of scope

1. **Website rendering changes** - This RFC focuses on generating JSON/CUE; website rendering remains unchanged
2. **Vector-specific functions** - Focus is VRL stdlib only (see Future work for Vector adoption)

**Note**: Migration of existing VRL stdlib documentation is IN SCOPE and detailed in the Feasibility section below.

## Pain

### Current state

**Manual documentation maintenance**:
- Documentation is written in separate CUE files: `website/cue/reference/remap/functions/*.cue`
- Each function requires manual creation of a `.cue` file with:
  - Function description
  - Parameter specifications (name, type, required, enum values)
  - Return type information
  - Examples with expected output
  - Error conditions
  - Categories and metadata

**Problems with current approach**:

1. **Cross-repository coordination required**: Contributors must create PRs in both VRL repo (function) and Vector repo (docs)
2. **Documentation drift**: Code changes in VRL repo don't automatically update docs in Vector repo
3. **Missing documentation**: New functions may be added to VRL without corresponding docs in Vector
4. **High contribution friction**: External contributors may not have access or knowledge to update both repos
5. **Inconsistent examples**: Examples in docs may not reflect actual function behavior
6. **Duplicate maintenance**: Information exists in both Rust code (VRL) and CUE files (Vector)
7. **Review burden**: PRs must be coordinated and reviewed across two repositories
8. **Testing gap**: Examples in documentation are not validated by VRL's test suite

### Example of current manual process

When adding a VRL stdlib function today, developers must work across **two repositories**:

#### In VRL repository:

1. **Implement the function** in Rust (e.g., `src/stdlib/parse_json.rs`):
```rust
impl Function for ParseJson {
    fn identifier(&self) -> &'static str { "parse_json" }

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
}
```

2. **Create PR in VRL repository** with function implementation and tests

#### In Vector repository (separate PR):

3. **Manually create** a CUE documentation file (e.g., `website/cue/reference/remap/functions/parse_json.cue`):
```cue
package metadata

remap: functions: parse_json: {
    category: "Parse"
    description: "Parses the value as JSON."

    arguments: [
        {
            name:        "value"
            description: "The string representation of the JSON to parse."
            required:    true
            type: ["string"]
        },
        {
            name:        "max_depth"
            description: "Number of layers to parse for nested JSON-formatted documents."
            required:    false
            type: ["integer"]
        },
    ]
    internal_failure_reasons: [
        "value is not a valid JSON-formatted payload.",
    ]
    return: types: ["boolean", "integer", "float", "string", "object", "array", "null"]

    examples: [
        {
            title: "Parse JSON"
            source: #"""
                parse_json!("{\"key\": \"val\"}")
                """#
            return: key: "val"
        },
    ]
}
```

4. **Create separate PR in Vector repository** with CUE documentation
5. **Coordinate review and merging** of both PRs
6. **Keep both in sync** when making changes to the function

#### The pain points:

- **Two PRs required**: Contributors must navigate two different repositories with different workflows
- **Coordination overhead**: Both PRs must be kept in sync and merged in the right order
- **Access barriers**: External contributors may not have permissions or familiarity with Vector repo
- **Documentation lag**: Function may be merged in VRL before docs are added in Vector
- **Duplication**: Same information (parameter types, examples) written in two places
- **No validation**: Changes to function in VRL don't trigger docs update in Vector

This cross-repository coordination is the core pain point this RFC aims to solve.

## Proposal

### Solution

Implement an **automated documentation generation system** that:

1. **Extracts documentation from Rust source code** using minimal annotations (`#[vrl_doc]`)
2. **Auto-generates JSON file** (`vrl-stdlib-doc.json`) containing all function documentation - **no manual JSON writing**
3. **Transforms JSON to CUE files** matching existing documentation format (Vector consumes this)
4. **Integrates with test suite** to validate documentation examples

**Key workflow**: Developer adds `#[vrl_doc]` attribute → runs script → `vrl-stdlib-doc.json` is auto-generated → commit both code and JSON

### Architecture overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     VRL Function Source Code                     │
│         (src/stdlib/ in VRL repository)                          │
│                                                                   │
│  - Function trait implementations                                │
│  - Structured documentation attributes/comments                  │
│  - Examples integrated with tests                                │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         │ (1) Parse & Extract
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│              Documentation Extraction Tool                       │
│             (Rust binary using syn/proc_macro)                   │
│                                                                   │
│  - Parse Rust AST to find Function implementations               │
│  - Extract metadata from trait methods                           │
│  - Parse documentation attributes                                │
│  - Validate completeness                                         │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         │ (2) Generate
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│           Intermediate JSON Schema                               │
│         (vrl-functions-doc.json)                                 │
│                                                                   │
│  {                                                                │
│    "functions": [                                                 │
│      {                                                            │
│        "name": "get_secret",                                      │
│        "category": "System",                                      │
│        "description": "...",                                      │
│        "parameters": [...],                                       │
│        "examples": [...],                                         │
│        "return_type": [...]                                       │
│      }                                                            │
│    ]                                                              │
│  }                                                                │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         │ (3) Transform
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│           CUE File Generator                                     │
│      (Transforms JSON to CUE format)                             │
│                                                                   │
│  - Reads intermediate JSON                                       │
│  - Generates .cue files per function                             │
│  - Maintains existing CUE structure                              │
│  - Formats output consistently                                   │
└────────────────────────┬─────────────────────────────────────────┘
                         │
                         │ (4) Output
                         │
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│        Generated CUE Documentation Files                         │
│     (website/cue/reference/remap/functions/*.cue)                │
│                                                                   │
│  - One file per function                                         │
│  - Ready for website consumption                                 │
│  - Validated and consistent                                      │
└─────────────────────────────────────────────────────────────────┘
```

### Documentation extraction approach: Hybrid extraction

The proposed approach combines automatic extraction from existing `Function` trait implementations with minimal supplementary attributes for metadata not present in the trait:

**Example implementation:**

```rust
#[derive(Clone, Copy, Debug)]
#[vrl_doc(
    category = "Parse",
    description = "Parses the value as JSON.",
    notices = ["Only JSON types are returned. If you need to convert a string into a timestamp, consider the parse_timestamp function."]
)]
pub struct ParseJson;

impl Function for ParseJson {
    fn identifier(&self) -> &'static str {
        "parse_json"  // ← Automatically extracted
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",        // ← Automatically extracted
                kind: kind::BYTES,       // ← Automatically extracted
                required: true,          // ← Automatically extracted
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
            title: "Parse JSON",                              // ← Automatically extracted
            source: r#"parse_json!("{\"key\": \"val\"}")"#,  // ← Automatically extracted
            result: Ok(value!({key: "val"})),                 // ← Automatically extracted
        }]
    }

    fn compile(/* ... */) -> Compiled { /* ... */ }
}

impl FunctionExpression for ParseJsonFn {
    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::bytes()
            .fallible()      // ← Automatically extracted
            .infallible()    // ← Automatically extracted
    }
}
```

**What gets automatically extracted:**
- ✅ Function name from `identifier()`
- ✅ Parameter names, types, required/optional from `parameters()`
- ✅ Example code and expected results from `examples()`
- ✅ Return types from `type_def()`
- ✅ Fallibility from `TypeDef::fallible()`/`infallible()`
- ✅ Purity from `TypeDef::impure()`

**What requires manual annotation (`#[vrl_doc]`):**
- Category classification (Parse, Encode, String, etc.)
- Human-readable description
- Notices/warnings
- Parameter descriptions (optional, can enhance extracted info)
- Deprecation markers

**Why this approach:**
1. **Minimal changes**: Only add 2-3 line attribute to struct
2. **DRY principle**: Don't duplicate information already in trait implementation
3. **Type safety**: Parameter types, return types extracted from actual code
4. **Incremental adoption**: Works even without attributes (generates basic docs)
5. **Validation**: Examples are already executable code in the codebase

### Intermediate JSON format

The tool **automatically generates** a JSON file (`vrl-stdlib-doc.json`) containing documentation for all functions. This intermediate format is consumable by multiple systems (Vector docs, VRL playground, Observability Pipelines, etc.):

**Example output** (auto-generated from VRL function code):

```json
{
  "schema_version": "1.0.0",
  "generated_at": "2025-10-27T10:00:00Z",
  "functions": [
    {
      "name": "get_secret",
      "category": "System",
      "description": "Returns the secret value for the provided key from Vector's secret store.",
      "notices": [
        "Secrets must be configured in Vector's configuration file."
      ],
      "arguments": [
        {
          "name": "key",
          "description": "The secret key to retrieve from the secret store.",
          "type": ["string"],
          "required": true,
          "default": null,
          "enum": null
        }
      ],
      "return_type": {
        "types": ["string", "null"],
        "description": "The secret value if found, null otherwise."
      },
      "internal_failure_reasons": [
        "Secret not found."
      ],
      "examples": [
        {
          "title": "Get the datadog api key",
          "source": "get_secret(\"datadog_api_key\")",
          "result": {
            "type": "value",
            "value": "secret value"
          },
          "input": null,
          "output": null
        }
      ],
      "is_fallible": true,
      "is_pure": true,
      "deprecated": false
    },
    {
      "name": "parse_bytes",
      "category": "Parse",
      "description": "Parses the value into a human-readable bytes format specified by unit and base.",
      "arguments": [
        {
          "name": "value",
          "description": "The string of the duration with either binary or SI unit.",
          "type": ["string"],
          "required": true,
          "default": null,
          "enum": null
        },
        {
          "name": "unit",
          "description": "The output units for the byte.",
          "type": ["string"],
          "required": true,
          "default": null,
          "enum": {
            "B": "Bytes",
            "kiB": "Kilobytes (1024 bytes)",
            "MiB": "Megabytes (1024 ** 2 bytes)",
            "GiB": "Gigabytes (1024 ** 3 bytes)",
            "TiB": "Terabytes (1024 gigabytes)",
            "PiB": "Petabytes (1024 ** 2 gigabytes)",
            "EiB": "Exabytes (1024 ** 3 gigabytes)",
            "kB": "Kilobytes (1 thousand bytes in SI)",
            "MB": "Megabytes (1 million bytes in SI)",
            "GB": "Gigabytes (1 billion bytes in SI)",
            "TB": "Terabytes (1 thousand gigabytes in SI)",
            "PB": "Petabytes (1 million gigabytes in SI)",
            "EB": "Exabytes (1 billion gigabytes in SI)"
          }
        },
        {
          "name": "base",
          "description": "The base for the byte, either 2 or 10.",
          "type": ["string"],
          "required": false,
          "default": 2,
          "enum": null
        }
      ],
      "return_type": {
        "types": ["float"],
        "description": null
      },
      "internal_failure_reasons": [
        "value is not a properly formatted bytes."
      ],
      "examples": [
        {
          "title": "Parse bytes (kilobytes)",
          "source": "parse_bytes!(\"1024KiB\", unit: \"MiB\")",
          "result": {
            "type": "value",
            "value": 1.0
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

### Implementation phases

#### Phase 1: Core extraction tool (Weeks 1-2)

**Deliverables**:
- Rust binary tool that parses VRL function source files
- AST parsing using `syn` crate to find `impl Function` blocks
- Extraction of basic metadata:
  - Function name from `identifier()`
  - Parameters from `parameters()`
  - Examples from `examples()`
  - Return type from `type_def()` in FunctionExpression trait
- Output intermediate JSON format
- Initial annotation attribute macro (`#[vrl_doc]`)

**Testing**:
- Unit tests for AST parsing
- Integration tests with sample functions
- Validation that JSON schema is well-formed

#### Phase 2: Rich documentation support (Weeks 3-4)

**Deliverables**:
- Support for extended attributes:
  - `category` classification
  - `description` text
  - `notices` array
  - Parameter descriptions
  - Enum value documentation
  - Deprecation markers
- Automatic inference where possible:
  - Fallibility from `TypeDef::fallible()`
  - Purity from `TypeDef::impure()`
  - Return types from `type_def()` method
- Validation and linting:
  - Warn on missing documentation
  - Error on invalid categories
  - Check example completeness

#### Phase 3: CUE file generation (Weeks 5-6)

**Deliverables**:
- CUE file generator that transforms JSON to CUE format
- Template system for CUE file structure
- Proper formatting and indentation
- Support for all CUE documentation features:
  - Multi-line descriptions
  - Nested example structures
  - Enum value documentation
  - Cross-references and URLs
- Integration with Vector build process

**Testing**:
- Compare generated CUE against existing manual CUE files
- Validate CUE syntax correctness
- Ensure website rendering works correctly

#### Phase 4: Test integration (Weeks 7-8)

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

#### Phase 5: Migration and rollout (Weeks 9-10)

**Deliverables**:
- Annotate all VRL stdlib functions
- Generate JSON documentation for all functions
- Update contributor documentation in VRL repo
- CI/CD integration for automated generation
- Deprecation of manual CUE file editing

### Implementation details

#### Tool structure

Located in VRL repository:

```
crates/vrl-doc/           # New crate in VRL repo
├── Cargo.toml
├── src/
│   ├── main.rs                 # CLI entry point
│   ├── lib.rs                  # Library exports
│   ├── extractor/
│   │   ├── mod.rs              # Extraction orchestration
│   │   ├── ast.rs              # AST parsing with syn
│   │   ├── function.rs         # Function trait analysis
│   │   ├── attributes.rs       # Attribute macro parsing
│   │   └── validation.rs       # Documentation validation
│   ├── schema/
│   │   ├── mod.rs              # Schema definitions
│   │   ├── function.rs         # Function schema struct
│   │   ├── parameter.rs        # Parameter schema struct
│   │   └── json.rs             # JSON serialization
│   ├── generator/
│   │   ├── mod.rs              # Generator orchestration
│   │   ├── cue.rs              # CUE file generation
│   │   └── templates.rs        # CUE templates
│   └── macros/
│       ├── mod.rs              # Proc macro exports
│       └── vrl_doc.rs          # #[vrl_doc] attribute macro
└── tests/
    ├── extractor_tests.rs
    ├── generator_tests.rs
    └── fixtures/
        └── sample_stdlib_functions.rs
```

#### CLI interface

```bash
# Extract documentation from VRL stdlib functions
cargo run --bin vrl-doc extract \
  --input src/stdlib \
  --output vrl-stdlib-doc.json

# Generate CUE files from JSON (for Vector's website)
cargo run --bin vrl-doc generate \
  --input vrl-stdlib-doc.json \
  --output ../vector/website/cue/reference/remap/functions/ \
  --format cue

# Combined extraction and generation
cargo run --bin vrl-doc build \
  --stdlib-dir src/stdlib \
  --output ../vector/website/cue/reference/remap/functions/

# Validate documentation completeness
cargo run --bin vrl-doc validate \
  --stdlib-dir src/stdlib
```

#### Integration with VRL build

Create new script `scripts/generate_docs.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Generate VRL stdlib documentation as JSON
cargo run --bin vrl-doc build \
  --stdlib-dir src/stdlib \
  --output vrl-stdlib-doc.json

echo "✓ Generated vrl-stdlib-doc.json"
```

Create new script `scripts/check_docs.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Validate documentation completeness
cargo run --bin vrl-doc validate --stdlib-dir src/stdlib

# Generate docs and check if they match committed version
cargo run --bin vrl-doc build \
  --stdlib-dir src/stdlib \
  --output /tmp/vrl-docs-check.json

if ! git diff --exit-code vrl-stdlib-doc.json; then
  echo "❌ Generated docs differ from committed docs"
  echo "Run './scripts/generate_docs.sh' and commit the changes"
  exit 1
fi

echo "✓ Documentation is up-to-date"
```

Add to CI pipeline (`.github/workflows/`):

```yaml
- name: Check VRL documentation is up-to-date
  run: ./scripts/check_docs.sh
```

#### Integration with Vector build

Vector can consume the JSON output from VRL:

```makefile
# In Vector's Makefile
.PHONY: update-vrl-docs
update-vrl-docs:
	# Copy JSON from VRL repo (or fetch from release artifact)
	cp ../vrl/vrl-stdlib-doc.json scripts/vrl-stdlib-doc.json
	# Generate CUE files from JSON
	cargo run --bin vrl-doc-cue-gen \
		--input scripts/vrl-stdlib-doc.json \
		--output website/cue/reference/remap/functions/
```

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

### Why intermediate JSON format?

1. **Flexibility**: Multiple consumers can use the same data (Vector docs, VRL playground, Observability Pipelines)
2. **Stability**: JSON schema provides versioned contract between extraction and generation
3. **Debugging**: Intermediate format can be inspected and validated independently
4. **Extensibility**: New output formats can consume same JSON without changing VRL code

### Why start in VRL repo?

This RFC focuses on VRL stdlib functions first because:
1. **Core problem location**: The GitHub issue was filed in the VRL repo, identifying this as the primary pain point
2. **Broader impact**: VRL stdlib functions are used by Vector, VRL playground, Observability Pipelines, and other consumers
3. **Single source of truth**: All ~150+ core VRL functions are defined in one repository
4. **Largest benefit**: The bulk of documentation maintenance burden comes from stdlib functions
5. **Natural ownership**: VRL repo owns both the function implementations and their documentation

## Drawbacks

1. **Initial migration cost**: All existing functions need annotations
2. **Build complexity**: Adds another build step and tool to maintain
3. **Learning curve**: Contributors need to learn annotation syntax
4. **Attribute macro complexity**: Proc macros can be difficult to debug
5. **Potential duplication**: Some information exists in both trait impl and attributes
6. **Cross-repo coordination**: Eventually needs adoption in VRL repo for full benefit

## Prior art

### Similar systems in other projects

1. **Rust's `rustdoc`**
   - Generates documentation from doc comments and code
   - Inspiration for attribute-based approach
   - Shows viability of code-as-source-of-truth

2. **Vector's config schema generation**
   - Already generates JSON schema for Vector configuration
   - Uses similar approach of extracting from Rust structs
   - Proves feasibility within Vector codebase

3. **OpenAPI/Swagger**
   - Generates API documentation from annotated code
   - Shows value of intermediate format (OpenAPI JSON/YAML)

4. **Python's Sphinx and docstrings**
   - Documentation extracted from code
   - Multiple output formats from single source

5. **JSDoc**
   - JavaScript documentation from structured comments
   - Demonstrates annotation-based approach

## Alternatives

### Alternative 1: Keep manual documentation

**Description**: Continue current approach of manual CUE file maintenance.

**Pros**:
- No implementation cost
- Maximum flexibility in documentation
- No tool maintenance

**Cons**:
- Documentation drift continues
- High maintenance burden
- Inconsistent quality
- Examples not validated

**Decision**: Rejected - this perpetuates the current pain points.

### Alternative 2: Generate minimal docs only

**Description**: Only auto-generate basic function signatures, keep descriptions manual.

**Pros**:
- Smaller implementation scope
- Preserves human-written descriptions
- Less code annotation needed

**Cons**:
- Partial solution - still requires manual maintenance
- Doesn't solve documentation drift
- Examples still not tested

**Decision**: Rejected - doesn't fully address the problem.

### Alternative 3: Attribute-based annotations (Full attributes)

**Description**: Use comprehensive attributes for all documentation, not just metadata:

```rust
#[derive(Clone, Copy, Debug)]
#[vrl_function(
    category = "Parse",
    description = "Parses the value as JSON.",
)]
pub struct ParseJson;

impl Function for ParseJson {
    #[vrl_param(
        name = "value",
        description = "The string representation of the JSON to parse.",
        type = "string",
        required = true,
    )]
    #[vrl_param(
        name = "max_depth",
        description = "Number of layers to parse for nested JSON-formatted documents.",
        type = "integer",
        required = false,
    )]
    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter { keyword: "value", kind: kind::BYTES, required: true },
            Parameter { keyword: "max_depth", kind: kind::INTEGER, required: false },
        ]
    }

    // ... more attributes for examples, return types, etc.
}
```

**Pros**:
- Complete documentation in attributes
- No need to parse trait implementations
- Clear, explicit documentation

**Cons**:
- Massive duplication (parameter types specified twice: attribute + code)
- Higher maintenance burden
- More verbose code
- Attributes can drift from actual implementation
- Violates DRY (Don't Repeat Yourself) principle

**Decision**: Rejected - duplicates information already present in code. The hybrid approach extracts from code instead.

### Alternative 4: Doc comments with structured format

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
- No new syntax to learn
- Works with existing Rust tooling (`rustdoc`)
- Familiar to Rust developers

**Cons**:
- Free-form text harder to parse reliably
- No compile-time validation of structure
- May require complex parsing logic with fragile regex/parsing
- Easy to have inconsistent formatting
- Duplicates type information (comments vs. code)

**Decision**: Rejected - too fragile to parse reliably and duplicates type information. Hybrid approach extracts types from code.

### Alternative 5: Generate Rust code from CUE files (reverse direction)

**Description**: Keep CUE as source of truth, generate Rust function stubs/validation.

**Pros**:
- CUE files remain authoritative
- Could validate function implementations match docs

**Cons**:
- Rust is the actual source of truth for behavior
- CUE lacks type information and compile-time validation
- Would require significant changes to VRL architecture
- Can't capture information only in code (e.g., type inference)

**Decision**: Rejected - Rust must be source of truth.

### Alternative 6: Use Rust's built-in `rustdoc`

**Description**: Use standard `rustdoc` with custom output format.

**Pros**:
- Well-tested existing tool
- Standard Rust documentation approach

**Cons**:
- `rustdoc` targets Rust API documentation, not VRL user-facing docs
- Can't generate JSON/CUE format needed
- Doesn't capture VRL-specific metadata (categories, VRL examples)
- Would need significant customization

**Decision**: Rejected - not designed for this use case.

### Alternative 7: Runtime reflection

**Description**: Use runtime reflection to discover function metadata.

**Pros**:
- No code changes needed
- Dynamic discovery

**Cons**:
- Significant runtime overhead
- VRL functions are compile-time constructs
- Can't run at build time without complex runtime harness
- Rust's reflection is limited

**Decision**: Rejected - not feasible in Rust's type system.

## Outstanding questions

1. **How do we handle Vector-specific VRL functions (defined in Vector repo)?**
   - **Answer**: Out of scope for this RFC. Focus is VRL stdlib functions only. See Future work section for Vector adoption.

2. **What happens when VRL function implementations change but docs aren't updated?**
   - **Answer**: CI checks will fail if generated docs differ from committed docs, forcing updates.

3. **How do we version the JSON schema for breaking changes?**
   - **Answer**: Include `schema_version` field in JSON. Tooling can support multiple versions.

4. **Can this work with VRL functions defined outside Vector/VRL repos?**
   - **Answer**: Yes, as long as they use the standard `Function` trait and annotations.

5. **How do we handle complex examples that require external state (e.g., enrichment tables)?**
   - **Answer**: Examples can include `input` and `output` sections for stateful examples, similar to existing CUE format.

6. **What about localization/internationalization of documentation?**
   - **Answer**: Out of scope. See Future work section.

7. **How do we prevent malicious code in documentation attributes?**
   - **Answer**: Attributes are compile-time only, no code execution. Standard code review applies.

## Feasibility

This proposal is technically feasible using well-established Rust tooling and patterns.

### Technical approach

**AST parsing with `syn`**

Rust's `syn` crate provides robust AST parsing capabilities used by many production tools (`serde_derive`, `cargo-doc`, etc.). We can parse VRL function source files to extract metadata:

```rust
// Parse Rust source file
let syntax_tree: syn::File = syn::parse_file(&source_code)?;

// Find impl Function blocks
for item in syntax_tree.items {
    if let syn::Item::Impl(impl_block) = item {
        if implements_function_trait(&impl_block) {
            // Extract from trait methods
            extract_identifier(&impl_block);    // fn identifier() -> &str
            extract_parameters(&impl_block);    // fn parameters() -> &[Parameter]
            extract_examples(&impl_block);      // fn examples() -> &[Example]
        }
    }
}
```

This is the same pattern used by derive macros throughout the Rust ecosystem.

**Hybrid extraction minimizes duplication**

Information already present in the `Function` trait implementation can be extracted automatically:
- Function name from `identifier()` method
- Parameter types from `parameters()` array
- Examples from `examples()` array
- Return types from `type_def()` method

Only metadata NOT in the trait requires manual annotation:
```rust
#[vrl_doc(
    category = "Parse",
    description = "Parses the value as JSON."
)]
pub struct ParseJson;
```

This minimizes work and keeps code as the source of truth.

**JSON as intermediate format**

Generating JSON provides:
- **Decoupling**: VRL generates docs, Vector/others consume independently
- **Flexibility**: Multiple consumers (Vector docs, VRL playground, Observability Pipelines)
- **Versioning**: Schema version field allows evolution
- **Validation**: Existing manual CUE docs provide validation baseline

### Existing similar systems

1. **Vector's config schema generation** - Already generates JSON schema from Rust code using similar AST parsing
2. **`serde_derive`** - Derives serialization from Rust structs using `syn` to parse AST
3. **`cargo-doc`** - Extracts documentation from Rust code using `rustdoc` parser
4. **Rust compiler's diagnostics** - Uses `syn` to parse code and provide rich error messages

These prove the technical approach is sound and well-supported.

### Migration validation

The ~150 existing manual CUE documentation files in Vector provide a **built-in validation baseline**:

1. Annotate VRL functions incrementally
2. Generate documentation automatically
3. Compare against existing manual docs
4. Investigate discrepancies:
   - Generated wrong → fix extraction logic
   - Manual wrong/outdated → proves value of automation!
   - Both valid → establish convention

**Target**: >90% exact matches between generated and manual docs.

This approach de-risks the migration by using existing docs as ground truth.

### Open questions

1. **How do we handle complex return types?** - Extract from `type_def()` method which already encodes this
2. **What about stateful examples?** - Use input/output format similar to existing CUE examples
3. **How to version JSON schema?** - Include `schema_version` field, tooling supports multiple versions
4. **Performance with 150+ functions?** - AST parsing is fast; `syn` is production-proven; should be <1 second

These are solvable implementation details, not fundamental blockers.

## Future work

This RFC focuses on VRL stdlib functions only. Future enhancements could include:

**Adoption by other projects:**
- Vector adoption for Vector-specific VRL functions (~7 functions in `lib/vector-vrl/functions/`)
- VRL playground integration for live examples with auto-complete
- Observability Pipelines integration

**Additional output formats:**
- Markdown documentation
- HTML standalone docs site
- OpenAPI-style schema
- Generate documentation website directly from VRL repo (not requiring Vector)

**Enhanced features:**
- IDE integration via Language Server Protocol (LSP)
- Localization/internationalization support
- Documentation versioning across VRL releases
- Automated migration tool for adding annotations to existing functions

**Extended metadata:**
- Performance characteristics (time/space complexity)
- Security considerations
- Version when function was added/changed

## Success metrics

1. **Documentation coverage**: 100% of VRL stdlib functions have auto-generated documentation
2. **Documentation freshness**: JSON is always auto-generated from code (enforced by CI)
3. **Example accuracy**: 100% of examples pass when executed as tests
4. **Developer efficiency**: Time to document new function reduced by 50%+
5. **CI reliability**: Documentation checks catch drift in 100% of cases
6. **Adoption**: Vector adopts similar approach for their functions within 6 months
7. **Consumer benefit**: VRL playground, Observability Pipelines consume JSON schema

## References

- [VRL Issue #280: Function documentation auto-generation](https://github.com/vectordotdev/vrl/issues/280)
- [Vector VRL Functions](https://github.com/vectordotdev/vector/tree/master/lib/vector-vrl/functions)
- [Vector Website CUE Documentation](https://github.com/vectordotdev/vector/tree/master/website/cue/reference/remap/functions)
- [VRL Repository](https://github.com/vectordotdev/vrl)
- [Rust syn crate](https://docs.rs/syn/) - AST parsing
- [CUE Language](https://cuelang.org/) - Configuration language
