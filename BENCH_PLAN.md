# KeyString Optimization Benchmark Plan

## Background

VRL's `Value::Object` stores its keys as `KeyString`, a newtype wrapper around a
string type. Currently this wraps `String`. We are evaluating alternative backing
types (`CompactString`, `EcoString`) that offer small-string optimization (inline
storage for short strings, avoiding heap allocation) and/or cheaper clones
(reference counting instead of full copy).

### The problem

Changing the `KeyString` backing type should be a transparent optimization, but
benchmark results show surprising interactions with VRL's flat `ObjectMap`
implementation. To understand why, we need targeted benchmarks that isolate the
different layers where `KeyString` is used.

### What we've learned so far

1. **The flat ObjectMap delivers +29% throughput** on a VRL-heavy Vector
   regression benchmark (`datadog_agent_remap_blackhole`). This win comes from
   contiguous memory layout and cheaper structural clones, not from KeyString
   changes.

2. **EcoString (16 bytes, refcounted, 15-byte inline) hurts the flat map by
   ~10%.** Its `as_str()` has a double-branch (inline vs spilled check) that
   penalizes the flat map's linear scan, which calls `as_str()` on every key for
   every lookup.

3. **CompactString (24 bytes, non-refcounted, 24-byte inline) is neutral on the
   flat map.** Its `as_str()` has a single discriminant check, close enough to
   `String`'s direct deref. Its `From<String>` is zero-copy (can take ownership
   of the String's heap buffer), unlike EcoString which always copies.

4. **Copy elimination (removing gratuitous `String` intermediaries in KeyString
   construction) shows -0.8% — statistical noise.** This is because with
   `String`-backed `KeyString`, the optimizer already collapses `s.to_string().into()`
   into the same code as `s.into()`. The fixes become load-bearing only with an
   SSO string type where `From<&str>` can inline short strings.

5. **The regression benchmark is lookup-dominated.** The VRL program does ~6 path
   lookups and ~4 path inserts per event, constructing only one small 2-key
   object. It does not exercise parse-heavy stdlib functions (`parse_syslog`,
   `parse_grok`, `flatten`, etc.) at all.

### The KeyString roundtrip problem

Compiled VRL expressions store paths as `OwnedValuePath`, which contains
`OwnedSegment::Field(KeyString)` — a pre-allocated key. But the path system
unifies compiled paths and runtime string paths behind a single iterator type
(`BorrowedSegment::Field(Cow<str>)`). This forces the compiled path to:

1. Convert `KeyString` → `&str` (via `as_str()`)
2. Wrap in `Cow::Borrowed(&str)`
3. In `crud/insert.rs`, convert back to `KeyString` (allocating a new one)

This means every path insert reconstructs a `KeyString` that already existed.
For reads (`get`, `get_mut`, `remove`), only the `&str` is needed so there's no
wasted allocation, but the conversion through `Cow` still happens.

### How Vector deserializes events

The Datadog Agent source (`datadog_agent_remap_blackhole` benchmark):

1. **Outer parse**: `serde_json::from_slice::<Vec<LogMsg>>()` deserializes into a
   Datadog-specific struct with typed fields (`message: Bytes`, `hostname: Bytes`,
   etc.). No `KeyString` construction here — serde matches JSON keys against
   struct fields at compile time.

2. **Event building**: Vector moves `LogMsg` fields onto a `LogEvent` using known
   field names. `KeyString` values are constructed from `&str` literals in
   Vector's code.

3. **Inner parse** (only if JSON decoding is configured): The `message` bytes are
   parsed via `serde_json::from_slice::<serde_json::Value>()`, then converted to
   `vrl::Value` via `From<serde_json::Value>`. Object keys go `String → KeyString`
   (from serde_json's `Map<String, Value>`). This does NOT use `Value`'s
   `Deserialize` impl — it uses the `From` conversion.

   `Value` also has a hand-written `Deserialize` impl where `visit_map`
   deserializes keys directly as `KeyString` (calling the inner type's
   `visit_str`/`visit_string`). This path is more efficient because it skips the
   `String` intermediate, but Vector doesn't use it today for JSON decoding.

---

## Benchmark Groups

### Bench 1: `keystring_micro`

**Purpose**: Ground truth for KeyString construction costs. Directly measures the
operations we're optimizing, with no surrounding noise. Use this to compare
`String` vs `CompactString` vs `EcoString` head-to-head.

**Cases**:

| Case | Operation | What it tests |
|------|-----------|---------------|
| `from_short_str` | `KeyString::from("hostname")` (8 bytes) | `From<&str>` — inline path for SSO types |
| `from_long_str` | `KeyString::from("upstream_response_time")` (22 bytes) | `From<&str>` — near inline limit |
| `from_string_short` | `KeyString::from(String::from("hostname"))` | `From<String>` — CompactString can take ownership |
| `from_string_long` | `KeyString::from(String::from("upstream_response_time"))` | `From<String>` — zero-copy for CompactString, copy for EcoString |
| `from_cow_borrowed` | `KeyString::from(Cow::Borrowed("hostname"))` | The path traversal conversion |
| `clone_short` | `short_key.clone()` | Clone cost — memcpy for CompactString, refcount for EcoString |
| `clone_long` | `long_key.clone()` | Clone cost at larger sizes |
| `roundtrip` | `KeyString` → `.as_str()` → `KeyString::from(s)` | The OwnedSegment → BorrowedSegment → KeyString path |

**Implementation notes**:
- Use `criterion::black_box` to prevent the optimizer from eliding the
  construction.
- For `from_string_*` cases, use `iter_batched` to construct a fresh `String`
  each iteration (since `From<String>` consumes it).

---

### Bench 2: `path_ops`

**Purpose**: Isolate the cost of path traversal through VRL's path system.
Specifically, measure the difference between `OwnedValuePath` (compiled VRL
expressions) and JIT string paths, and show the KeyString roundtrip cost.

**Setup**: Build a realistic event — a `Value::Object` with ~15 fields using
typical field names (`message`, `hostname`, `status`, `severity`, `facility`,
`appname`, `timestamp`, `procid`, `source_type`, `service`, `env`, `version`,
`trace_id`, `span_id`, `tags`). Values should be `Value::Bytes` of realistic
lengths.

**Cases**:

| Case | Operation | What it tests |
|------|-----------|---------------|
| `owned_path_get` | `value.get(&owned_path)` for a mid-depth field | Compiled path lookup: `KeyString → &str → Cow → as_ref() → map.get()` |
| `owned_path_insert` | `value.insert(&owned_path, v)` | Compiled path insert: includes the KeyString roundtrip |
| `owned_path_nested_get` | `value.get(&owned_path)` for `foo.bar.baz` (3 levels) | Multiple roundtrips per operation |
| `owned_path_nested_insert` | `value.insert(&owned_path, v)` for 3 levels | Multiple KeyString reconstructions |
| `jit_path_get` | `value.get("field_name")` | JIT lookup: `&str → Cow::Borrowed → as_ref() → map.get()` |
| `jit_path_insert` | `value.insert("field_name", v)` | JIT insert: `&str → Cow::Borrowed → KeyString::from(Cow)` |

**Implementation notes**:
- Pre-compile `OwnedValuePath` values using `parse_value_path()` outside the
  benchmark loop.
- Use `iter_batched` for insert benchmarks since they mutate the value.
- The delta between `owned_path_insert` and `jit_path_insert` shows the cost of
  the KeyString roundtrip (reconstruct from `&str` of an existing `KeyString`
  vs construct from a fresh `&str`). These should be approximately equal — if
  `owned_path_insert` is slower, it means the roundtrip has overhead beyond just
  the `From<&str>` call.

**What to look for**:
- `owned_path_insert` vs `jit_path_insert`: if these are roughly equal, the
  roundtrip isn't adding overhead beyond the `KeyString::from(&str)` cost. If
  owned is slower, something in the `OwnedSegment → BorrowedSegment` conversion
  is adding cost (e.g., the `Cow` wrapper, iterator overhead).
- How get vs insert scales: gets should be cheaper since they don't construct
  `KeyString`. The delta is the per-insert `KeyString` construction cost.

---

### Bench 3: `vrl_programs`

**Purpose**: End-to-end compiled VRL execution against realistic events. This is
our correlation check — `remap_fields` should approximately track the Vector
regression benchmark (`datadog_agent_remap_blackhole`). The other cases exercise
construction-heavy paths where copy elimination and CompactString should show
measurable gains.

**Setup**: Use VRL's `compile()` function to compile programs, then
`Runtime::resolve()` to execute them. Build realistic input events as
`TargetValue { value, metadata }`.

**Cases**:

| Case | VRL Program | What it tests |
|------|-------------|---------------|
| `remap_fields` | `.hostname = "vector"; if .status == "warning" { .thing = upcase(.hostname) } ...` (the exact benchmark VRL) | Correlation with Vector regression bench. Lookup-dominated. |
| `parse_syslog` | `. = parse_syslog!(.message)` | Stdlib object construction — builds ~9-key ObjectMap from parsed fields. Exercises KeyString construction from `&str` literals. |
| `parse_and_flatten` | `parsed = parse_syslog!(.message); . = flatten(parsed)` | Object construction + compound key construction via `format!()`. |
| `object_construction` | `.result = { "method": .method, "path": .path, "status": .status, "duration": .duration, "host": .host, "service": .service, "env": .env, "version": .version, "trace_id": .trace_id, "message": .message }` | Constructs a 10-field object literal. Exercises `Object::resolve()` which clones compiled `KeyString` values. |

**Implementation notes**:
- Compile programs once, outside the benchmark loop.
- For `remap_fields`, build the input event to match what the Datadog Agent
  source would produce (15+ fields, `status` field with varying values to
  exercise different branches).
- For `parse_syslog`, set `.message` to a valid RFC5424 syslog line.
- Use `iter_batched` with a fresh clone of the input event each iteration.

**What to look for**:
- `remap_fields` should be the least sensitive to KeyString type (it's
  lookup-dominated). If CompactString shows a gain here, it's likely from the
  path insert improvement.
- `parse_syslog` and `object_construction` should show the biggest delta between
  String and CompactString, since they construct many KeyStrings.
- `parse_and_flatten` stresses `format!().into()` for compound keys — this is
  `From<String>` where CompactString is zero-copy.

---

### Bench 4: `json_deser`

**Purpose**: Measure Value construction from JSON, covering both the path Vector
actually uses and a potential optimization.

**Setup**: A JSON string representing a realistic 15-field Datadog-style log
event. Include a nested object (e.g., `"http": {"method": "GET", "status": 200}`)
to exercise recursive key construction.

**Cases**:

| Case | Operation | What it tests |
|------|-----------|---------------|
| `via_serde_json_value` | `serde_json::from_str::<serde_json::Value>()` then `Value::from()` | The actual Vector path. Keys go `String → KeyString`. |
| `direct_deserialize` | `serde_json::from_str::<Value>()` | Direct deserialization. Keys go through `KeyString`'s `Deserialize` → inner type's `visit_str`. No `String` intermediate. |

**Implementation notes**:
- Use the same JSON string for both cases.
- Include a variety of key lengths: short (`"host"`, `"env"`), medium
  (`"hostname"`, `"severity"`), long (`"upstream_response_time"`).

**What to look for**:
- The delta between the two paths shows the cost of the `serde_json::Value`
  intermediate. With `String`-backed KeyString, both paths allocate per key.
  With CompactString, the direct path can inline short keys via `visit_str`
  without allocating, while the `serde_json::Value` path still allocates a
  `String` (though CompactString's `From<String>` is zero-copy).
- If the delta is significant, it suggests Vector should consider switching to
  direct `Value` deserialization in its JSON codec.

---

## Running the benchmarks

```bash
# All KeyString benchmarks
cargo bench --bench keystring --features "default,test"

# Compare against a baseline (run on main first, then on your branch)
cargo bench --bench keystring --features "default,test" -- --save-baseline main
# ... switch branches ...
cargo bench --bench keystring --features "default,test" -- --baseline main
```

## Interpreting results for Vector-level impact

The Vector regression benchmark runs at ~500K events/sec with the BTreeMap
baseline. Each event goes through:

1. **Deserialization** (bench 4): ~15 KeyString constructions for a typical event
2. **VRL execution** (bench 3 `remap_fields`): ~6 lookups + ~4 inserts
3. **Serialization** to blackhole sink: reads all fields

To estimate Vector-level impact from local bench results:

- Multiply per-operation savings from bench 1/2 by the operation count above
- Compare that to the per-event time from bench 3 `remap_fields`
- If the per-operation savings are <1% of per-event time, the change won't
  register in the Vector benchmark
- If >5%, it should be visible

The `remap_fields` bench is the most direct predictor. If it shows a 2%
improvement locally, expect roughly a 1-3% improvement in the Vector regression
benchmark (local benches have less noise and overhead than the full pipeline).
