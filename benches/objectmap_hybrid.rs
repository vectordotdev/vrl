//! Hybrid ObjectMap design experiments.
//!
//! The `objectmap_cliff` benchmark showed that the Flat backend's linear-scan
//! lookup cost produces large isolated-op cliffs, but the O(1) `EcoVec`-clone
//! property keeps it competitive in realistic flows. This bench explores
//! hybrid designs that attempt to keep the cheap-clone property while
//! closing the lookup gap:
//!
//!   * `btree`            — baseline BTreeMap<KeyString, Value>
//!   * `flat`             — unsorted EcoVec with linear-scan lookup (current
//!                          Flat backend)
//!   * `sorted`           — sorted EcoVec with binary-search lookup (proposed)
//!   * `threshold(N)`     — sorted EcoVec up to N entries, then BTree (also
//!                          proposed, but harder to justify unless `sorted`
//!                          loses to BTree somewhere)
//!
//! These are implemented inline in the bench so we can iterate without
//! churning the library. Results tell us whether a hybrid design is worth
//! productionizing and where to put the threshold.
//!
//! Run with:
//!   cargo bench --bench objectmap_hybrid --features "default,test"

use std::fmt;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ecow::EcoVec;
use vrl::value::{KeyString, Value};

// ---------------------------------------------------------------------------
// Candidate map representations
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[allow(dead_code)] // Threshold variants retained for historical reference.
enum HybridMap {
    BTree(std::collections::BTreeMap<KeyString, Value>),
    /// Unsorted; insertion-order `push`; linear-scan lookup. Matches the
    /// current `Flat` backend semantics.
    FlatEco(EcoVec<(KeyString, Value)>),
    /// Always-sorted-by-key; binary-search lookup.
    SortedEco(EcoVec<(KeyString, Value)>),
    /// Starts as SortedEco; promotes to BTree once `len >= threshold` on
    /// insert. Demotion is never attempted (the typical pipeline doesn't
    /// shrink back).
    Threshold(ThresholdMap),
    /// Like FlatEco, but remembers whether entries are sorted. `get` sorts
    /// the vec on first call after mutations; subsequent gets binary-search.
    /// Size: EcoVec (16B) + bool + padding = 24B — same as the BTreeMap
    /// variant, so adding this to the real ObjectMap enum would not grow it.
    LazySorted(LazySorted),
}

#[derive(Clone)]
struct LazySorted {
    entries: EcoVec<(KeyString, Value)>,
    /// Invariant: when `true`, `entries` is sorted by key and contains no
    /// duplicate keys. When `false`, entries is in insertion order and may
    /// contain duplicates (newer key-value overrides older).
    sorted: bool,
}

#[derive(Clone)]
#[allow(dead_code)]
struct ThresholdMap {
    threshold: usize,
    inner: ThresholdInner,
}

#[derive(Clone)]
#[allow(dead_code)]
enum ThresholdInner {
    Small(EcoVec<(KeyString, Value)>),
    Large(std::collections::BTreeMap<KeyString, Value>),
}

impl HybridMap {
    fn new_btree() -> Self {
        Self::BTree(std::collections::BTreeMap::new())
    }
    fn new_flat() -> Self {
        Self::FlatEco(EcoVec::new())
    }
    fn new_sorted() -> Self {
        Self::SortedEco(EcoVec::new())
    }
    #[allow(dead_code)]
    fn new_threshold(threshold: usize) -> Self {
        Self::Threshold(ThresholdMap {
            threshold,
            inner: ThresholdInner::Small(EcoVec::new()),
        })
    }
    fn new_lazy_sorted() -> Self {
        Self::LazySorted(LazySorted {
            entries: EcoVec::new(),
            sorted: true, // vacuously
        })
    }

    /// Mutable-self get: required by the `LazySorted` variant (first lookup
    /// after mutation triggers a sort). All other variants ignore `&mut`.
    /// This is a bench-level concession — a production `LazySorted` would
    /// need interior mutability (e.g. an `UnsafeCell<bool>` + `UnsafeCell`
    /// or a dedicated `get_mut` API).
    fn get(&mut self, key: &str) -> Option<&Value> {
        match self {
            Self::BTree(m) => m.get(key),
            Self::FlatEco(v) => v.iter().find(|(k, _)| k.as_str() == key).map(|(_, v)| v),
            Self::SortedEco(v) => match v.binary_search_by(|(ek, _)| ek.as_str().cmp(key)) {
                Ok(idx) => Some(&v[idx].1),
                Err(_) => None,
            },
            Self::Threshold(t) => match &t.inner {
                ThresholdInner::Small(v) => {
                    match v.binary_search_by(|(ek, _)| ek.as_str().cmp(key)) {
                        Ok(idx) => Some(&v[idx].1),
                        Err(_) => None,
                    }
                }
                ThresholdInner::Large(m) => m.get(key),
            },
            Self::LazySorted(ls) => {
                if !ls.sorted {
                    // Sort with dedup: inserts are pure appends, so there
                    // may be multiple entries per key. Stable sort keeps
                    // insertion order within duplicates; we then collapse
                    // adjacent dupes keeping the LAST (i.e. most recent)
                    // value per key.
                    let slice = ls.entries.make_mut();
                    slice.sort_by(|a, b| a.0.cmp(&b.0));
                    // Collapse adjacent duplicates keeping the last. Walk
                    // in reverse; move each unique key to the front.
                    let mut write = 0usize;
                    let mut i = 0usize;
                    while i < slice.len() {
                        // Find end of the run of equal keys starting at i.
                        let mut j = i + 1;
                        while j < slice.len() && slice[j].0 == slice[i].0 {
                            j += 1;
                        }
                        // The last element of the run (index j-1) is the
                        // most recent insertion for this key (stable sort).
                        if write != j - 1 {
                            slice.swap(write, j - 1);
                        }
                        write += 1;
                        i = j;
                    }
                    // Truncate the tail.
                    let new_len = write;
                    // EcoVec doesn't expose truncate directly on make_mut's
                    // slice, but the EcoVec itself does via `truncate`.
                    ls.entries.truncate(new_len);
                    ls.sorted = true;
                }
                match ls.entries.binary_search_by(|(ek, _)| ek.as_str().cmp(key)) {
                    Ok(idx) => Some(&ls.entries[idx].1),
                    Err(_) => None,
                }
            }
        }
    }

    fn insert(&mut self, key: KeyString, value: Value) -> Option<Value> {
        match self {
            Self::BTree(m) => m.insert(key, value),
            Self::FlatEco(v) => {
                if let Some(pos) = v.iter().position(|(k, _)| *k == key) {
                    Some(std::mem::replace(&mut v.make_mut()[pos].1, value))
                } else {
                    v.push((key, value));
                    None
                }
            }
            Self::SortedEco(v) => {
                match v.binary_search_by(|(ek, _)| ek.as_str().cmp(key.as_str())) {
                    Ok(idx) => Some(std::mem::replace(&mut v.make_mut()[idx].1, value)),
                    Err(idx) => {
                        v.insert(idx, (key, value));
                        None
                    }
                }
            }
            Self::Threshold(t) => {
                let threshold = t.threshold;
                // Promote if we'd be inserting the `threshold`-th entry (as a
                // new key). Cheap check: only promote on Err from binary_search
                // when len == threshold.
                match &mut t.inner {
                    ThresholdInner::Small(v) => {
                        match v.binary_search_by(|(ek, _)| ek.as_str().cmp(key.as_str())) {
                            Ok(idx) => Some(std::mem::replace(&mut v.make_mut()[idx].1, value)),
                            Err(idx) => {
                                if v.len() >= threshold {
                                    // Promote: move entries into a BTreeMap.
                                    let old = std::mem::replace(v, EcoVec::new());
                                    let mut m = std::collections::BTreeMap::new();
                                    for (k, val) in old {
                                        m.insert(k, val);
                                    }
                                    m.insert(key, value);
                                    t.inner = ThresholdInner::Large(m);
                                    None
                                } else {
                                    v.insert(idx, (key, value));
                                    None
                                }
                            }
                        }
                    }
                    ThresholdInner::Large(m) => m.insert(key, value),
                }
            }
            Self::LazySorted(ls) => {
                // Two-path insert:
                //   Sorted state → binary_search; update in place if found,
                //   push+mark-unsorted if not (stays sorted only if the new
                //   key happens to sort at the end).
                //   Unsorted state → pure append. No dedup; the next `get`
                //   will sort+dedup. This gives O(1) inserts during bulk
                //   builds (the worst case for `sorted`) at the cost of
                //   potentially keeping duplicate entries around until the
                //   next read triggers cleanup.
                if ls.sorted {
                    match ls
                        .entries
                        .binary_search_by(|(ek, _)| ek.as_str().cmp(key.as_str()))
                    {
                        Ok(idx) => {
                            Some(std::mem::replace(&mut ls.entries.make_mut()[idx].1, value))
                        }
                        Err(idx) => {
                            if idx == ls.entries.len() {
                                ls.entries.push((key, value));
                                // sorted remains true
                            } else {
                                ls.entries.push((key, value));
                                ls.sorted = false;
                            }
                            None
                        }
                    }
                } else {
                    // Append without dedup. Duplicates will be collapsed
                    // on next read.
                    ls.entries.push((key, value));
                    // We don't know if the prior inserts produced a
                    // duplicate; return None. Update semantics are still
                    // correct because sort+dedup keeps the last value per
                    // key, and the caller of insert-returning-Option almost
                    // never actually uses the returned prior value.
                    None
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    BTree,
    Flat,
    Sorted,
    Lazy,
}

impl Variant {
    fn make(self) -> HybridMap {
        match self {
            Self::BTree => HybridMap::new_btree(),
            Self::Flat => HybridMap::new_flat(),
            Self::Sorted => HybridMap::new_sorted(),
            Self::Lazy => HybridMap::new_lazy_sorted(),
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BTree => write!(f, "btree"),
            Self::Flat => write!(f, "flat"),
            Self::Sorted => write!(f, "sorted"),
            Self::Lazy => write!(f, "lazy"),
        }
    }
}

const VARIANTS: [Variant; 4] = [
    Variant::BTree,
    Variant::Flat,
    Variant::Sorted,
    Variant::Lazy,
];

/// Widths chosen to span below and above the predicted threshold crossovers.
const WIDTHS: [usize; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn key(i: usize) -> KeyString {
    KeyString::from(format!("http.request.header_{i:04}"))
}

fn miss_key() -> KeyString {
    KeyString::from("http.request.header_MISSING")
}

fn new_key() -> KeyString {
    KeyString::from("http.request.header_NEW")
}

fn build(variant: Variant, width: usize) -> HybridMap {
    let mut map = variant.make();
    for i in 0..width {
        map.insert(key(i), Value::from(i as i64));
    }
    map
}

struct ReadKeys {
    mid: KeyString,
    miss: KeyString,
    mixed: [KeyString; 6],
}

fn read_keys(width: usize) -> ReadKeys {
    let pick = |num: usize, den: usize| key((width.saturating_sub(1) * num) / den.max(1));
    ReadKeys {
        mid: pick(1, 2),
        miss: miss_key(),
        mixed: [
            pick(0, 1),
            pick(1, 8),
            pick(3, 8),
            pick(1, 2),
            pick(5, 8),
            pick(7, 8),
        ],
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_get_hit_mid(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/get_hit_mid");
    for variant in VARIANTS {
        for &width in &WIDTHS {
            // For LazySorted, trigger a priming lookup so the map is sorted
            // before the timed loop. This captures the *steady-state* get
            // cost (all variants already-ready). `build_then_first_read`
            // below covers the cold-sort case.
            let mut map = build(variant, width);
            let keys = read_keys(width);
            let _ = map.get(keys.mid.as_str());
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    black_box(map.get(keys.mid.as_str()));
                });
            });
        }
    }
}

fn bench_get_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/get_miss");
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut map = build(variant, width);
            let keys = read_keys(width);
            let _ = map.get(keys.miss.as_str());
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    black_box(map.get(keys.miss.as_str()));
                });
            });
        }
    }
}

fn bench_insert_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/insert_update");
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut map = build(variant, width);
            let keys = read_keys(width);
            let id = format!("{variant}/width={width:04}");
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

fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/build");
    group.sample_size(30);
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let keys: Vec<KeyString> = (0..width).map(key).collect();
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let mut map = variant.make();
                    for (i, k) in keys.iter().enumerate() {
                        map.insert(k.clone(), Value::from(i as i64));
                    }
                    black_box(map);
                });
            });
        }
    }
}

/// Same as `bench_build` but with keys inserted in a scrambled (non-sorted)
/// order. For the `sorted` variant each insertion now requires shifting the
/// tail, so this is the worst case for vector-based sorted maps. BTree is
/// insertion-order-agnostic.
fn bench_build_shuffled(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/build_shuffled");
    group.sample_size(30);
    for variant in VARIANTS {
        for &width in &WIDTHS {
            // Cheap deterministic scramble: reverse and interleave halves.
            // Produces a permutation that puts every insertion near the
            // middle, maximizing shift cost for sorted vectors.
            let mut keys: Vec<KeyString> = (0..width).map(key).collect();
            let half = keys.len() / 2;
            let (a, b) = keys.split_at(half);
            let scrambled: Vec<KeyString> = a
                .iter()
                .zip(b.iter().rev())
                .flat_map(|(x, y)| [x.clone(), y.clone()])
                .chain(if keys.len() % 2 == 1 {
                    Some(keys.last().unwrap().clone())
                } else {
                    None
                })
                .collect();
            keys = scrambled;
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let mut map = variant.make();
                    for (i, k) in keys.iter().enumerate() {
                        map.insert(k.clone(), Value::from(i as i64));
                    }
                    black_box(map);
                });
            });
        }
    }
}

fn bench_realistic_event(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/realistic_event");
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut base = build(variant, width);
            // Prime the sort for LazySorted so the base is already sorted
            // (simulates a long-lived source event whose sort state
            // amortizes across many fan-out clones).
            let keys = read_keys(width);
            let _ = base.get(keys.mid.as_str());
            let new_k = new_key();
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let mut map = base.clone();
                    for k in &keys.mixed[..5] {
                        black_box(map.get(k.as_str()));
                    }
                    black_box(map.get(keys.miss.as_str()));
                    black_box(map.insert(keys.mid.clone(), Value::from(1i64)));
                    black_box(map.insert(new_k.clone(), Value::from(2i64)));
                    black_box(map);
                });
            });
        }
    }
}

fn bench_realistic_event_readonly(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/realistic_event_readonly");
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut base = build(variant, width);
            let keys = read_keys(width);
            let _ = base.get(keys.mid.as_str());
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b| {
                b.iter(|| {
                    let mut map = base.clone();
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

/// Cold-path benchmark for LazySorted: build a map of N entries (shuffled
/// insertion order, so Sorted pays its O(N^2) shift cost) and then perform
/// a single lookup. For LazySorted, the first lookup is where the O(N log N)
/// sort happens — this bench captures the amortized "build + activation"
/// cost that steady-state get benches miss.
fn bench_build_then_first_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/build_then_first_read");
    group.sample_size(30);
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut keys: Vec<KeyString> = (0..width).map(key).collect();
            let half = keys.len() / 2;
            let (a, b) = keys.split_at(half);
            let scrambled: Vec<KeyString> = a
                .iter()
                .zip(b.iter().rev())
                .flat_map(|(x, y)| [x.clone(), y.clone()])
                .chain(if keys.len() % 2 == 1 {
                    Some(keys.last().unwrap().clone())
                } else {
                    None
                })
                .collect();
            keys = scrambled;
            let read_keys_ = read_keys(width);
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b_| {
                b_.iter(|| {
                    let mut map = variant.make();
                    for (i, k) in keys.iter().enumerate() {
                        map.insert(k.clone(), Value::from(i as i64));
                    }
                    // Single lookup — for LazySorted this triggers the
                    // internal sort.
                    black_box(map.get(read_keys_.mid.as_str()));
                    black_box(map);
                });
            });
        }
    }
}

/// Build a map of N entries in shuffled order, then do 10 lookups. This
/// amortizes LazySorted's sort cost across multiple reads (the typical
/// pattern for a source event that the pipeline reads many fields from).
fn bench_build_then_many_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap_hybrid/build_then_many_reads");
    group.sample_size(30);
    for variant in VARIANTS {
        for &width in &WIDTHS {
            let mut keys: Vec<KeyString> = (0..width).map(key).collect();
            let half = keys.len() / 2;
            let (a, b) = keys.split_at(half);
            let scrambled: Vec<KeyString> = a
                .iter()
                .zip(b.iter().rev())
                .flat_map(|(x, y)| [x.clone(), y.clone()])
                .chain(if keys.len() % 2 == 1 {
                    Some(keys.last().unwrap().clone())
                } else {
                    None
                })
                .collect();
            keys = scrambled;
            let read_keys_ = read_keys(width);
            let id = format!("{variant}/width={width:04}");
            group.bench_function(BenchmarkId::from_parameter(&id), |b_| {
                b_.iter(|| {
                    let mut map = variant.make();
                    for (i, k) in keys.iter().enumerate() {
                        map.insert(k.clone(), Value::from(i as i64));
                    }
                    for k in &read_keys_.mixed {
                        black_box(map.get(k.as_str()));
                    }
                    black_box(map.get(read_keys_.miss.as_str()));
                    black_box(map.get(read_keys_.mid.as_str()));
                    black_box(map.get(read_keys_.mid.as_str()));
                    black_box(map.get(read_keys_.mid.as_str()));
                    black_box(map);
                });
            });
        }
    }
}

/// Prints the size (and memory layout) of each candidate type. Called
/// before the bench suite runs so the numbers are visible in bench output.
fn print_sizes() {
    use std::mem::size_of;
    eprintln!("---- type sizes ----");
    eprintln!(
        "ObjectMap (real):        {} bytes",
        size_of::<vrl::value::ObjectMap>()
    );
    eprintln!("HybridMap (this bench):  {} bytes", size_of::<HybridMap>());
    eprintln!(
        "EcoVec<(K,V)>:           {} bytes",
        size_of::<EcoVec<(KeyString, Value)>>()
    );
    eprintln!("LazySorted struct:       {} bytes", size_of::<LazySorted>());
    eprintln!(
        "BTreeMap<K,V>:           {} bytes",
        size_of::<std::collections::BTreeMap<KeyString, Value>>()
    );
    eprintln!("---------------------");
}

fn configured() -> Criterion {
    print_sizes();
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50)
}

criterion_group!(
    name = benches;
    config = configured();
    targets =
        bench_get_hit_mid,
        bench_get_miss,
        bench_insert_update,
        bench_build,
        bench_build_shuffled,
        bench_build_then_first_read,
        bench_build_then_many_reads,
        bench_realistic_event,
        bench_realistic_event_readonly,
);
criterion_main!(benches);
