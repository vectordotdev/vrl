# ObjectMap Refactor Plan

## Goal

Make `ObjectMap` safe to evolve into an enum-backed storage type without changing user-visible behavior during the refactor. The immediate objective is to remove all `BTreeMap`-specific API leaks from `ObjectMap`, introduce a second backend variant, and keep the tree green throughout while backend selection remains fixed.

## Current State

This branch now contains the following mechanical steps:

- `ObjectMap` is an enum in `src/value/value.rs`
- `ObjectMap` has `BTree` and `Index` variants
- `ObjectMap`-owned iterator and entry wrapper types no longer leak `btree_map::*`
- immediate `Entry` and iterator call sites have been migrated to those wrappers
- targeted tests now exercise the `Index` variant directly

Current behavior is still intentionally conservative:

- `ObjectMap::new()`, `FromIterator`, and array conversions use a cached `VRL_OBJECT_MAP` env var
- supported values today are `btree` / `btreemap` and `index` / `indexmap`
- the default remains `BTree` when the env var is unset or invalid
- explicit constructors still exist for direct tests and bench code:
  - `ObjectMap::new_btree()`
  - `ObjectMap::new_index()`
  - `ObjectMap::with_backend(...)`

What still blocks an enum-backed implementation is that `ObjectMap` exposes:

- `From<BTreeMap<...>>` and `PartialEq<BTreeMap<...>>`

The first three bullets above have been addressed. The remaining work is mostly about tightening construction-time compatibility shims, validating semantics, and deciding how backend selection should eventually occur.

## Current Bench Workflow

For quick local A/B runs without code edits:

```bash
cargo bench --features 'default test' --bench stdlib parse_query_string -- --noplot
VRL_OBJECT_MAP=index cargo bench --features 'default test' --bench stdlib parse_query_string -- --noplot
```

The same pattern works for any existing benchmark that builds objects through `ObjectMap::new()`, `collect::<ObjectMap>()`, or `ObjectMap::from([...])`.

There are also call sites that directly import `std::collections::btree_map::Entry` or store `btree_map::Iter` in structs.

## Constraints

- Keep the repository compiling after each slice.
- Prefer small, monotonic steps over broad rewrites.
- Preserve iteration and serialization order while `ObjectMap` is still backed by `BTreeMap`.
- Do not introduce runtime backend switching until the API no longer leaks `BTreeMap`.
- Validate each slice with compiler and targeted tests before proceeding.

## Phases

### Phase 0: Plan and Baseline

Deliverables:

- this plan in `REFACTOR.md`
- confirm current leak points and call sites

Validation:

- no code changes beyond the plan

### Phase 1: Add ObjectMap-Owned Wrapper Types

Objective:

Define `ObjectMap`-owned wrapper types around the current `BTreeMap` APIs while keeping the backend unchanged.

Deliverables:

- introduce wrapper types in `src/value/value.rs`:
  - `ObjectMapIter<'a>`
  - `ObjectMapIterMut<'a>`
  - `ObjectMapKeys<'a>`
  - `ObjectMapValues<'a>`
  - `ObjectMapValuesMut<'a>`
  - `ObjectMapIntoIter`
  - `ObjectMapIntoKeys`
  - `ObjectMapIntoValues`
  - `ObjectMapEntry<'a>`
  - `ObjectMapOccupiedEntry<'a>`
  - `ObjectMapVacantEntry<'a>`
- change `ObjectMap::{iter, iter_mut, keys, values, values_mut, into_keys, into_values, entry}` to return wrappers
- change `IntoIterator for ObjectMap`, `&ObjectMap`, and `&mut ObjectMap` to use wrapper iterators

Notes:

- this phase keeps `ObjectMap` backed by `BTreeMap`
- wrapper types should initially expose only the methods the repo actually needs
- avoid over-modeling the full std entry API

Validation checkpoints:

- `cargo check --lib`
- fix compile fallout immediately in the same slice

### Phase 2: Migrate Immediate Wrapper Call Sites

Objective:

Update call sites that currently depend on concrete `btree_map` types.

Expected files:

- `src/parsing/xml.rs`
- `src/parsing/query_string.rs`
- `src/stdlib/parse_key_value.rs`
- `src/stdlib/flatten.rs`
- `src/stdlib/keys.rs`
- `src/stdlib/values.rs`
- any other compile fallout from Phase 1

Deliverables:

- replace imports of `std::collections::btree_map::Entry` with `crate::value::ObjectMapEntry`
- adjust pattern matches to use `ObjectMapEntry::{Occupied, Vacant}`
- convert `flatten` to store `ObjectMapIter<'a>` instead of `btree_map::Iter<'a, ...>`
- ensure `keys` and `values` use wrapper iterators

Validation checkpoints:

- `cargo check --lib`
- targeted tests:
  - `cargo test parsing::query_string`
  - `cargo test parse_key_value`
  - `cargo test parse_xml`
  - `cargo test flatten`

### Phase 3: Remove Remaining BTreeMap Leaks from Value Conversion Surface

Objective:

Reduce non-essential `BTreeMap` coupling outside the iterator and entry surface.

Expected areas:

- `src/value/value/convert.rs`
- `src/value/value/lua.rs`
- docs and comments in `src/value/value.rs`
- helper macros and tests where appropriate

Deliverables:

- evaluate whether `From<BTreeMap<KeyString, Value>> for ObjectMap` and `for Value` should remain temporarily or move behind crate-private helpers
- evaluate whether `PartialEq<BTreeMap<...>>` should remain during migration or be removed
- replace direct `BTreeMap` deserialization/construction where it materially leaks into `ObjectMap` semantics

Validation checkpoints:

- `cargo check --lib`
- targeted value tests:
  - `cargo test value::value`
  - `cargo test value::value::iter`
  - `cargo test value::value::path`

### Phase 4: Make ObjectMap Trait Impl Semantics Explicit

Objective:

Prepare for multiple backends by removing derives and trait behavior that assume a single concrete storage type.

Focus:

- `Hash`
- `PartialOrd`
- equality and ordering semantics
- serialization guarantees

Deliverables:

- decide whether to keep sorted iteration as an invariant or document backend-specific behavior
- if sorted iteration remains an invariant, encode it at the abstraction layer
- if not, update docs and user-visible functions accordingly
- replace derived trait impls with explicit impls if needed

Validation checkpoints:

- `cargo check --lib`
- targeted serialization and behavior tests

### Phase 5: Introduce Enum-Backed ObjectMap Internals

Objective:

Switch `ObjectMap` internals from:

- `struct ObjectMap(BTreeMap<...>)`

to something like:

- `enum ObjectMap { BTree(...), Alt(...) }`

Only begin this phase once the previous phases are green.

Deliverables:

- enum-backed `ObjectMap`
- add a second backend variant, initially without any switching mechanism
- wrapper iterators and entry types dispatch internally by variant
- constructor selection API:
  - explicit constructor
  - feature gate
  - or cached env-var-based default selection

Validation checkpoints:

- `cargo check --lib`
- targeted tests from earlier phases
- targeted tests that construct the non-default backend directly
- benchmark smoke test:
  - `cargo bench --bench kind`
  - any new object-map-focused benchmark added later

## Sequencing Strategy

The safest order is:

1. wrappers first
2. compile fallout next
3. behavior cleanup after the tree is green again
4. backend polymorphism last

This keeps each checkpoint narrow and makes it easy to stop and reassess if the abstraction becomes awkward.

## Known Hotspots

These are the files most likely to need immediate changes during the wrapper phase:

- `src/value/value.rs`
- `src/parsing/xml.rs`
- `src/parsing/query_string.rs`
- `src/stdlib/parse_key_value.rs`
- `src/stdlib/flatten.rs`
- `src/stdlib/keys.rs`
- `src/stdlib/values.rs`

These are secondary hotspots for follow-up cleanup:

- `src/value/value/convert.rs`
- `src/value/value/lua.rs`
- `src/stdlib/filter.rs`
- `src/stdlib/merge.rs`
- `src/stdlib/http_request.rs`
- `src/datadog/grok/parse_grok.rs`

## Checkpoint Policy

Do not stack multiple unvalidated abstraction changes at once.

After each phase:

1. run `cargo check --lib`
2. run the smallest relevant targeted tests
3. only then move to the next slice

If a phase widens unexpectedly, stop and split it into a smaller compilable step rather than pushing forward with a red tree.
