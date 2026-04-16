# Proposal: Optimized ObjectMap Representation

## Summary

We propose replacing the `ObjectMap` type alias (`type ObjectMap = BTreeMap<KeyString, Value>`) with a proper newtype struct, enabling aggressive internal optimization while preserving the existing API. The goal is to reduce pointer indirections and improve cache locality for the core data structure underlying Vector's `LogEvent`.

## Motivation

In Vector, every log event is ultimately stored as a VRL `Value::Object(ObjectMap)`. The current representation — a `BTreeMap<KeyString, Value>` — has several performance characteristics that are suboptimal for the actual workload:

- **Poor cache locality**: BTreeMap nodes are heap-allocated and pointer-linked. For typical log events (5-20 flat fields), tree traversal adds overhead compared to a flat, contiguous layout.
- **Deep pointer chains**: Accessing a string field in a LogEvent requires 5-6 pointer indirections: `LogEvent -> Arc<Inner> -> Value::Object(BTreeMap) -> BTreeMap node -> Value::Bytes(Bytes) -> heap data`.
- **Copy-on-write cost**: Vector wraps events in `Arc<Inner>` for cheap cloning. The first mutation after a clone copies the *entire* Value tree, including all BTreeMap nodes.

The path-based CRUD operations (`insert`, `get`, `remove` with nested paths) are implemented on `Value` in VRL, making VRL the right place to optimize — doing it downstream in Vector would leave the hottest code path (the remap transform) untouched.

## Current State

- `ObjectMap` is a bare type alias for `BTreeMap<KeyString, Value>` (defined in `src/value/value.rs`)
- `KeyString` is a newtype around `String`, explicitly documented as opaque with the comment: *"the underlying type is opaque and may change for efficiency"*
- The `ValueCollection` trait in `src/value/value/crud/mod.rs` already provides an abstraction over ObjectMap's CRUD operations (`get_value`, `insert_value`, `remove_value`, etc.)
- VRL has existing quickcheck property tests on Value operations

## Proposed Approach

### Phase 1: Newtype + Test Oracle

1. **Replace the type alias with a proper struct**: `pub struct ObjectMap(BTreeMap<KeyString, Value>)`. Implement the same traits (`IntoIterator`, `FromIterator`, `Index`, `Serialize`/`Deserialize`, etc.) so all existing code compiles.
2. **Build a comprehensive differential test harness**: The oracle is the current BTreeMap implementation. Generate random sequences of operations (insert, get, remove, iterate, serialize) and assert both implementations produce identical results. Extend the existing quickcheck infrastructure.
3. **Add benchmarks** for the actual hot operations: insert-then-serialize, bulk field iteration, nested path insert/get, clone-then-mutate.

### Phase 2: Optimize Internals

With the test harness in place, explore alternative internal representations:

- **Sorted `Vec<(KeyString, Value)>`**: Identical semantics to BTreeMap for small-to-medium maps, dramatically better cache locality. Binary search gives O(log n) lookup.
- **Small-map optimization**: Inline storage for maps below a threshold (e.g., 8-16 entries), avoiding heap allocation entirely for typical log events.
- **Arena-backed storage**: Store keys and values in a contiguous byte buffer with an index for O(log n) lookup. Reduces per-entry allocation overhead.

### Phase 3: Downstream Validation

- Bump VRL dependency in Vector
- Run Vector's full test suite and benchmarks
- Validate with real-world pipeline configurations

## Constraints

- **Sorted iteration order must be preserved.** BTreeMap's sorted key order is depended on by JSON serialization, VRL tests, and deterministic output.
- **Path-based CRUD semantics must be identical.** `insert(["a", "b", "c"], v)` must auto-create intermediate objects. `remove` with pruning must clean up empty parents.
- **Serde compatibility.** Serializes as JSON object, deserializes from JSON object.
- **VRL has consumers beyond Vector.** Any code that relies on ObjectMap being a BTreeMap (e.g., calling BTreeMap-specific methods not exposed through traits) will need updating.

## API Surface to Preserve

Based on analysis of actual usage across Vector (~320 direct ObjectMap references in 57 files) and VRL internals, the exercised API is:

| Operation | Notes |
|-----------|-------|
| `new()`, `default()` | Construction |
| `insert(key, value)` | Returns previous value |
| `get(key)` / `get_mut(key)` | Key lookup |
| `remove(key)` | Key removal |
| `contains_key(key)` | Existence check |
| `keys()` / `values()` / `iter()` / `iter_mut()` | Iteration |
| `len()` / `is_empty()` | Size queries |
| `into_iter()` / `from_iter()` | Conversion |
| `entry(key)` | Entry API (used in some places) |
| `extend(iter)` | Bulk insertion |
| `Serialize` / `Deserialize` | Serde support |
| `ValueCollection` trait methods | VRL CRUD dispatch |

## Risks

1. **Entry API complexity**: BTreeMap's `entry()` returns a specific `Entry` enum. A custom struct needs its own Entry type. This is mechanical but touches call sites.
2. **KeyString co-optimization**: KeyString (currently heap-allocated `String`) could benefit from small-string optimization or interning, but changing both types simultaneously increases risk. Recommend sequencing ObjectMap first.
3. **Nested map performance**: Nested objects create nested ObjectMaps. A flat representation only helps at each level individually — truly flat event storage (single allocation for the whole event) would require deeper changes to `Value` itself.
4. **BTreeMap method leakage**: Any code calling BTreeMap-specific methods (e.g., `range()`, `split_off()`) through the current type alias would break. Needs an audit of all usage sites.

## Expected Impact

The primary beneficiaries in Vector are:
- **Remap transform**: Executes VRL programs that call `insert`/`get`/`remove` per event — the single hottest path for field access
- **Sink serialization**: Every sink serializes events, iterating all fields
- **Reduce transform**: Iterates all fields of every event during aggregation
- **Event creation in sources**: Sequential field insertion when constructing events from raw data
