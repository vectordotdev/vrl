use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::fmt;
use std::hint::black_box;
use vrl::value::{ObjectMap, Value};

#[derive(Clone, Copy)]
struct FlatCase {
    width: usize,
}

#[derive(Clone, Copy)]
struct DepthCase {
    depth: usize,
    siblings: usize,
}

impl fmt::Display for FlatCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "width={}", self.width)
    }
}

impl fmt::Display for DepthCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "depth={},siblings={}", self.depth, self.siblings)
    }
}

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
            Self::Flat => ObjectMap::new(),
        }
    }
}

const BACKENDS: [Backend; 2] = [Backend::BTree, Backend::Flat];

const FLAT_CASES: [FlatCase; 3] = [
    FlatCase { width: 8 },
    FlatCase { width: 64 },
    FlatCase { width: 256 },
];

const DEPTH_ONLY_CASES: [DepthCase; 4] = [
    DepthCase {
        depth: 1,
        siblings: 0,
    },
    DepthCase {
        depth: 4,
        siblings: 0,
    },
    DepthCase {
        depth: 8,
        siblings: 0,
    },
    DepthCase {
        depth: 16,
        siblings: 0,
    },
];

const DEPTH_WITH_FANOUT_CASES: [DepthCase; 4] = [
    DepthCase {
        depth: 1,
        siblings: 8,
    },
    DepthCase {
        depth: 4,
        siblings: 8,
    },
    DepthCase {
        depth: 8,
        siblings: 8,
    },
    DepthCase {
        depth: 16,
        siblings: 8,
    },
];

fn flat_map(width: usize, backend: Backend) -> ObjectMap {
    let mut map = backend.new_map();
    for i in 0..width {
        map.insert(format!("key_{i}").into(), Value::from(i as i64));
    }
    map
}

/// Build a nested Value tree using ad-hoc construction (separate ObjectMap per level).
fn nested_value_adhoc(depth: usize, siblings: usize, backend: Backend) -> Value {
    fn build_level(level: usize, depth: usize, siblings: usize, backend: Backend) -> Value {
        let mut map = backend.new_map();

        for sibling in 0..siblings {
            map.insert(
                format!("sibling_{level}_{sibling}").into(),
                Value::from((level * 10 + sibling) as i64),
            );
        }

        if level + 1 == depth {
            map.insert("target".into(), Value::from(level as i64));
        } else {
            map.insert(
                format!("level_{level}").into(),
                build_level(level + 1, depth, siblings, backend),
            );
        }

        Value::Object(map)
    }

    build_level(0, depth.max(1), siblings, backend)
}

/// Build a nested Value tree using insert_child (child inherits parent backend).
fn nested_value_insert_child(depth: usize, siblings: usize, backend: Backend) -> Value {
    fn build_level(map: &mut ObjectMap, level: usize, depth: usize, siblings: usize) {
        for sibling in 0..siblings {
            map.insert(
                format!("sibling_{level}_{sibling}").into(),
                Value::from((level * 10 + sibling) as i64),
            );
        }

        if level + 1 == depth {
            map.insert("target".into(), Value::from(level as i64));
        } else {
            let child = map.insert_child(format!("level_{level}").into());
            build_level(child, level + 1, depth, siblings);
        }
    }

    let mut root = backend.new_map();
    let actual_depth = depth.max(1);
    build_level(&mut root, 0, actual_depth, siblings);
    Value::Object(root)
}

fn nested_target_path(depth: usize) -> String {
    let mut parts = Vec::with_capacity(depth.max(1));
    for level in 0..depth.saturating_sub(1) {
        parts.push(format!("level_{level}"));
    }
    parts.push("target".to_owned());
    parts.join(".")
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn benchmark_flat_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/flat_insert");

    for backend in BACKENDS {
        for case in FLAT_CASES {
            group.throughput(Throughput::Elements(case.width as u64));
            group.bench_with_input(
                BenchmarkId::new(backend.to_string(), case),
                &case,
                |b, case| {
                    b.iter_batched(
                        || flat_map(case.width, backend),
                        |mut map| {
                            black_box(map.insert("new_key".into(), Value::from(42)));
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
}

fn benchmark_flat_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/flat_get");

    for backend in BACKENDS {
        for case in FLAT_CASES {
            let map = flat_map(case.width, backend);
            // Look up a key near the middle
            let target = format!("key_{}", case.width / 2);
            group.bench_with_input(
                BenchmarkId::new(backend.to_string(), case),
                &(map, target),
                |b, (map, target)| {
                    b.iter(|| {
                        black_box(map.get(target.as_str()));
                    });
                },
            );
        }
    }
}

fn benchmark_nested_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/nested_get");

    for backend in BACKENDS {
        for case in DEPTH_ONLY_CASES {
            let value = nested_value_adhoc(case.depth, case.siblings, backend);
            let path = nested_target_path(case.depth);
            group.bench_with_input(
                BenchmarkId::new(backend.to_string(), case),
                &(value, path),
                |b, (value, path)| {
                    b.iter(|| {
                        black_box(value.get(path.as_str()));
                    });
                },
            );
        }
    }
}

fn benchmark_nested_get_with_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/nested_get_with_fanout");

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            let value = nested_value_adhoc(case.depth, case.siblings, backend);
            let path = nested_target_path(case.depth);
            group.bench_with_input(
                BenchmarkId::new(backend.to_string(), case),
                &(value, path),
                |b, (value, path)| {
                    b.iter(|| {
                        black_box(value.get(path.as_str()));
                    });
                },
            );
        }
    }
}

fn benchmark_nested_insert_with_fanout(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/nested_insert_with_fanout");

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            let base = nested_value_adhoc(case.depth, case.siblings, backend);
            let path = format!("{}.write_leaf", nested_target_path(case.depth));
            group.bench_with_input(
                BenchmarkId::new(backend.to_string(), case),
                &(base, path),
                |b, (base, path)| {
                    b.iter_batched(
                        || base.clone(),
                        |mut value| {
                            black_box(value.insert(path.as_str(), Value::from(99)));
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
}

/// Benchmarks construction of a nested tree.
/// Compares ad-hoc construction (each level creates a standalone ObjectMap)
/// vs insert_child (child allocated through parent API).
fn benchmark_nested_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/nested_build");

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            group.bench_with_input(
                BenchmarkId::new(format!("{}/adhoc", backend), case),
                &case,
                |b, case| {
                    b.iter(|| {
                        black_box(nested_value_adhoc(case.depth, case.siblings, backend));
                    });
                },
            );
        }
    }

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            group.bench_with_input(
                BenchmarkId::new(format!("{}/insert_child", backend), case),
                &case,
                |b, case| {
                    b.iter(|| {
                        black_box(nested_value_insert_child(case.depth, case.siblings, backend));
                    });
                },
            );
        }
    }
}

/// Benchmarks full round-trip: build a nested tree, then read from it.
/// Compares the two construction methods while measuring the same read path.
fn benchmark_build_then_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/build_then_read");

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            let path = nested_target_path(case.depth);

            group.bench_with_input(
                BenchmarkId::new(format!("{}/adhoc", backend), case),
                &(case, path),
                |b, (case, path)| {
                    b.iter(|| {
                        let value = nested_value_adhoc(case.depth, case.siblings, backend);
                        black_box(value.get(path.as_str()));
                    });
                },
            );
        }
    }

    for backend in BACKENDS {
        for case in DEPTH_WITH_FANOUT_CASES {
            let path = nested_target_path(case.depth);

            group.bench_with_input(
                BenchmarkId::new(format!("{}/insert_child", backend), case),
                &(case, path),
                |b, (case, path)| {
                    b.iter(|| {
                        let value = nested_value_insert_child(case.depth, case.siblings, backend);
                        black_box(value.get(path.as_str()));
                    });
                },
            );
        }
    }
}

/// Simulates Vector's fan-out pattern: clone an event, then mutate the clone.
/// This is where clone-on-write would show its benefit.
fn benchmark_clone_then_mutate(c: &mut Criterion) {
    let mut group = c.benchmark_group("objectmap/clone_then_mutate");

    for backend in BACKENDS {
        for case in FLAT_CASES {
            let map = flat_map(case.width, backend);
            let event = Value::Object(map);

            // Clone only
            group.bench_with_input(
                BenchmarkId::new(format!("{}/clone_only", backend), case),
                &event,
                |b, event| {
                    b.iter(|| {
                        black_box(event.clone());
                    });
                },
            );

            // Clone then insert 3 fields (simulates transform modifying a cloned event)
            group.bench_with_input(
                BenchmarkId::new(format!("{}/clone_insert_3", backend), case),
                &event,
                |b, event| {
                    b.iter(|| {
                        let mut cloned = event.clone();
                        cloned.insert("_new_field_1", Value::from(1));
                        cloned.insert("_new_field_2", Value::from(2));
                        cloned.insert("_new_field_3", Value::from(3));
                        black_box(cloned);
                    });
                },
            );

            // Clone then read 3 fields (simulates sink reading a cloned event)
            let target = format!("key_{}", case.width / 2);
            group.bench_with_input(
                BenchmarkId::new(format!("{}/clone_read_3", backend), case),
                &(event, target),
                |b, (event, target)| {
                    b.iter(|| {
                        let cloned = event.clone();
                        black_box(cloned.get(target.as_str()));
                        black_box(cloned.get("key_0"));
                        black_box(cloned.get("key_1"));
                    });
                },
            );
        }
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets =
        benchmark_flat_insert,
        benchmark_flat_get,
        benchmark_nested_get,
        benchmark_nested_get_with_fanout,
        benchmark_nested_insert_with_fanout,
        benchmark_nested_build,
        benchmark_build_then_read,
        benchmark_clone_then_mutate
);
criterion_main!(benches);
