//! Microbenchmarks aimed at finding the map-width at which the Flat (EcoVec)
//! `ObjectMap` backend begins to perform worse than the BTree backend.
//!
//! Primary axis: map width (number of entries in the map).
//! Secondary axes:
//!   - key length / shape (short identifier vs realistic dotted field name)
//!   - operation kind (hit lookup, miss lookup, update-insert, bulk build,
//!     and a mixed "realistic event" sequence that simulates a small VRL
//!     program)
//!
//! Access pattern is held roughly constant per bench (we hit a key near the
//! middle of the keyspace or a predictable pattern of keys) so runs are
//! comparable across widths.
//!
//! Run with:
//!   cargo bench --bench objectmap_cliff --features "default,test"
//!
//! NOTE: `ObjectMap::new()` selects a backend via the `VRL_OBJECT_MAP` env
//! var, defaulting to Flat. This bench forces `Flat` by setting that env
//! before any `ObjectMap` is constructed. `ObjectMap::new_btree()` is
//! unaffected.

use std::fmt;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::value::{KeyString, ObjectMap, Value};

// ---------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Backend {
    BTree,
    Flat,
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BTree => write!(f, "btree"),
            Self::Flat => write!(f, "flat"),
        }
    }
}

impl Backend {
    fn new_map(self) -> ObjectMap {
        match self {
            Self::BTree => ObjectMap::new_btree(),
            // `new()` honors VRL_OBJECT_MAP; we set it to "flat" in `main()`.
            Self::Flat => ObjectMap::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum KeyStyle {
    /// Short identifier-like keys ("k_0042"). Typical for synthetic tests
    /// and some well-behaved schemas. ~6-8 bytes.
    Short,
    /// Realistic dotted field names mixed with numeric disambiguation
    /// ("http.request.header_0042"). ~24-28 bytes. Realistic for log event
    /// metadata and flattened objects.
    Realistic,
}

impl fmt::Display for KeyStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short => write!(f, "short_key"),
            Self::Realistic => write!(f, "realistic_key"),
        }
    }
}

impl KeyStyle {
    fn make(self, i: usize) -> String {
        match self {
            Self::Short => format!("k_{i:04}"),
            Self::Realistic => format!("http.request.header_{i:04}"),
        }
    }
}

const BACKENDS: [Backend; 2] = [Backend::BTree, Backend::Flat];
const KEY_STYLES: [KeyStyle; 2] = [KeyStyle::Short, KeyStyle::Realistic];
/// Width sweep chosen to span small (where Flat should win) through large
/// (where BTree's O(log n) should dominate). Exponential, but dense enough
/// around the likely crossover (32-256) to read the cliff clearly.
const WIDTHS: [usize; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_map(width: usize, style: KeyStyle, backend: Backend) -> ObjectMap {
    let mut map = backend.new_map();
    for i in 0..width {
        map.insert(KeyString::from(style.make(i)), Value::from(i as i64));
    }
    map
}

/// Precompute keys the bench will exercise so `format!` is not on the hot path.
struct ReadKeys {
    /// The key right around the middle of the map (typical "average" hit).
    mid: KeyString,
    /// A key guaranteed to miss.
    miss: KeyString,
    /// Six realistic access keys distributed across the keyspace. Models a
    /// small VRL program's read pattern.
    mixed: [KeyString; 6],
}

fn read_keys(width: usize, style: KeyStyle) -> ReadKeys {
    // Miss key shares the style so string-comparison cost is representative.
    let miss_name = match style {
        KeyStyle::Short => "k_MISSING".to_owned(),
        KeyStyle::Realistic => "http.request.header_MISSING".to_owned(),
    };
    let pick = |frac_num: usize, frac_den: usize| {
        // Clamp to [0, width-1].
        let idx = (width.saturating_sub(1) * frac_num) / frac_den.max(1);
        KeyString::from(style.make(idx))
    };
    ReadKeys {
        mid: pick(1, 2),
        miss: KeyString::from(miss_name),
        mixed: [
            pick(0, 1), // first
            pick(1, 8), // near front
            pick(3, 8),
            pick(1, 2), // middle
            pick(5, 8),
            pick(7, 8), // near end
        ],
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Single `get` for a key that exists near the middle of the map.
/// This is the core read cliff: linear scan (Flat) vs log-n descent (BTree).
fn bench_get_hit_mid(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/get_hit_mid");

    for style in KEY_STYLES {
        for backend in BACKENDS {
            for &width in &WIDTHS {
                let map = build_map(width, style, backend);
                let keys = read_keys(width, style);
                let id = format!("{backend}/{style}/width={width:04}");
                group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                    b.iter(|| black_box(map.get(keys.mid.as_str())));
                });
            }
        }
    }
}

/// Single `get` for a key that does NOT exist. This is Flat's worst-case:
/// the full O(n) scan runs every time.
fn bench_get_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/get_miss");

    for style in KEY_STYLES {
        for backend in BACKENDS {
            for &width in &WIDTHS {
                let map = build_map(width, style, backend);
                let keys = read_keys(width, style);
                let id = format!("{backend}/{style}/width={width:04}");
                group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                    b.iter(|| black_box(map.get(keys.miss.as_str())));
                });
            }
        }
    }
}

/// Re-insert an existing key (in-place update). The insert path does a key
/// lookup and then replaces the value in place. Uses a persistent map so the
/// measurement excludes clone/drop cost (which would otherwise dominate
/// BTree's numbers and make the comparison unfair). The map's key set is
/// unchanged across iterations — only the value at `mid` is overwritten.
fn bench_insert_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/insert_update");

    for style in KEY_STYLES {
        for backend in BACKENDS {
            for &width in &WIDTHS {
                let mut map = build_map(width, style, backend);
                let keys = read_keys(width, style);
                let id = format!("{backend}/{style}/width={width:04}");
                group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                    let mut counter: i64 = 0;
                    b.iter(|| {
                        counter = counter.wrapping_add(1);
                        black_box(map.insert(keys.mid.clone(), Value::from(counter)));
                    });
                });
            }
        }
    }
}

/// Build a map of `width` entries from scratch. Captures the full-build cost,
/// dominated by per-insert dedup scan for Flat (O(n^2)) vs rebalancing for
/// BTree (O(n log n)).
fn bench_build_from_scratch(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/build");
    // Build cost scales with width; let criterion sample fewer big iterations.
    group.sample_size(30);

    for style in KEY_STYLES {
        for backend in BACKENDS {
            for &width in &WIDTHS {
                // Precompute key strings to keep the bench focused on the map.
                let keys: Vec<KeyString> =
                    (0..width).map(|i| KeyString::from(style.make(i))).collect();
                let id = format!("{backend}/{style}/width={width:04}");
                group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                    b.iter(|| {
                        let mut map = backend.new_map();
                        for (i, k) in keys.iter().enumerate() {
                            map.insert(k.clone(), Value::from(i as i64));
                        }
                        black_box(map);
                    });
                });
            }
        }
    }
}

/// Semi-realistic "one VRL program run against an event" sequence. Mirrors
/// what Vector's regression bench shows (~6 lookups + ~2 inserts per event),
/// applied to events of varying width (number of fields).
///
/// Per iteration, starting from a pre-built `base` map:
///   - clone the map (events are copy-on-write in Vector's hot path)
///   - 5 hits spread across the keyspace
///   - 1 miss
///   - 1 update (in-place insert of an existing key)
///   - 1 new insert (append)
///   - drop the clone
///
/// The clone is INSIDE the measured closure, so we capture BTree's O(n)
/// deep-copy cost and Flat's O(1) refcount bump (with the first mutation
/// triggering EcoVec's CoW). This matches how events actually flow through
/// a VRL pipeline.
fn bench_realistic_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/realistic_event");

    // Realistic keys only; short identifier benches are less representative
    // of real event schemas and this bench already produces many combinations.
    let style = KeyStyle::Realistic;

    for backend in BACKENDS {
        for &width in &WIDTHS {
            let base = build_map(width, style, backend);
            let keys = read_keys(width, style);
            let new_key = KeyString::from("http.request.header_NEW".to_owned());
            let id = format!("{backend}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let mut map = base.clone();
                    for k in &keys.mixed[..5] {
                        black_box(map.get(k.as_str()));
                    }
                    black_box(map.get(keys.miss.as_str()));
                    black_box(map.insert(keys.mid.clone(), Value::from(1i64)));
                    black_box(map.insert(new_key.clone(), Value::from(2i64)));
                    black_box(map);
                });
            });
        }
    }
}

/// Same sequence as `realistic_event` but WITHOUT any mutation — pure
/// read-only traversal of a cloned event. Captures the case where a VRL
/// program is a pure filter/projection (no `.x = y`). For Flat this avoids
/// triggering EcoVec's CoW, keeping the clone at O(1).
fn bench_realistic_event_readonly(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_cliff/realistic_event_readonly");

    let style = KeyStyle::Realistic;

    for backend in BACKENDS {
        for &width in &WIDTHS {
            let base = build_map(width, style, backend);
            let keys = read_keys(width, style);
            let id = format!("{backend}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let map = base.clone();
                    for k in &keys.mixed {
                        black_box(map.get(k.as_str()));
                    }
                    black_box(map.get(keys.miss.as_str()));
                    black_box(map);
                });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn configured() -> Criterion {
    // Keep total runtime reasonable: ~144 benches here (5 * 2 * 2 * 9 - 18 for
    // realistic-only). Default criterion settings would run ~20+ minutes.
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50)
}

fn force_flat_backend() {
    // Fix backend choice so bench results are meaningful regardless of the
    // user's shell env.
    // SAFETY: benches run single-threaded before any ObjectMap is constructed.
    unsafe { std::env::set_var("VRL_OBJECT_MAP", "flat") };
}

criterion_group!(
    name = benches;
    config = { force_flat_backend(); configured() };
    targets =
        bench_get_hit_mid,
        bench_get_miss,
        bench_insert_update,
        bench_build_from_scratch,
        bench_realistic_event,
        bench_realistic_event_readonly,
);
criterion_main!(benches);
