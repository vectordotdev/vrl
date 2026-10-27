//! Benchmarks for large objects: map operations across sizes, and VRL programs
//! that work on whole objects. Compiles against both the `BTreeMap` alias and the
//! flat `ObjectMap`, so the same file compares implementations.
#![allow(
    clippy::cast_possible_wrap,
    clippy::too_many_lines,
    clippy::format_collect,
    clippy::format_push_string
)]
use std::hint::black_box;

use bytes::Bytes;
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::compiler::runtime::Runtime;
use vrl::compiler::{Program, TargetValue, TimeZone, compile};
use vrl::value::{KeyString, ObjectMap, Secrets, Value};

const SIZES: [usize; 5] = [8, 32, 128, 512, 2048];
const VRL_SIZES: [usize; 3] = [32, 256, 1024];
const PIPELINE_SIZES: [usize; 5] = [8, 32, 256, 1024, 4096];

/// Deterministic permutation of `0..n` (LCG-driven Fisher-Yates).
fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    let mut s = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    for i in (1..n).rev() {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = (s >> 33) as usize % (i + 1);
        v.swap(i, j);
    }
    v
}

#[derive(Clone, Copy)]
enum KeyStyle {
    /// 10 bytes, inline for small-string keys.
    Short,
    /// ~45 bytes, heap allocated for any key type (flattened-path style).
    Long,
}

fn key(style: KeyStyle, i: usize) -> String {
    match style {
        KeyStyle::Short => format!("field_{i:04}"),
        KeyStyle::Long => format!("kubernetes.pod_labels.app_kubernetes_io_{i:04}"),
    }
}

fn keys(style: KeyStyle, n: usize, seed: u64) -> Vec<KeyString> {
    shuffled(n, seed)
        .into_iter()
        .map(|i| KeyString::from(key(style, i)))
        .collect()
}

fn value(i: usize) -> Value {
    if i.is_multiple_of(2) {
        Value::Integer(i as i64)
    } else {
        Value::Bytes(Bytes::from_static(b"some moderately sized string value"))
    }
}

fn build(keys: &[KeyString]) -> ObjectMap {
    let mut map = ObjectMap::new();
    for (i, k) in keys.iter().enumerate() {
        map.insert(k.clone(), value(i));
    }
    map
}

fn bench_map_ops(c: &mut Criterion) {
    for (style, style_name) in [(KeyStyle::Short, "short"), (KeyStyle::Long, "long")] {
        let mut g = c.benchmark_group(format!("large/{style_name}"));
        for &n in &SIZES {
            if matches!(style, KeyStyle::Long) && !(n == 32 || n == 512) {
                continue;
            }
            let ks = keys(style, n, 1);
            let lookup_order = keys(style, n, 2);
            let missing: Vec<KeyString> =
                (n..2 * n).map(|i| KeyString::from(key(style, i))).collect();
            let map = build(&ks);
            let json = serde_json::to_string(&map).unwrap();

            g.bench_with_input(BenchmarkId::new("insert_random", n), &n, |b, _| {
                b.iter(|| black_box(build(&ks)));
            });
            g.bench_with_input(BenchmarkId::new("collect_random", n), &n, |b, _| {
                b.iter(|| {
                    black_box(
                        ks.iter()
                            .enumerate()
                            .map(|(i, k)| (k.clone(), value(i)))
                            .collect::<ObjectMap>(),
                    )
                });
            });
            g.bench_with_input(BenchmarkId::new("get_hit_all", n), &n, |b, _| {
                b.iter(|| {
                    for k in &lookup_order {
                        black_box(map.get(k.as_str()));
                    }
                });
            });
            g.bench_with_input(BenchmarkId::new("get_miss_all", n), &n, |b, _| {
                b.iter(|| {
                    for k in &missing {
                        black_box(map.get(k.as_str()));
                    }
                });
            });
            g.bench_with_input(BenchmarkId::new("insert_remove_one", n), &n, |b, _| {
                // Steady-state single mutation of a large object (key sorts mid-map).
                let mut m = map.clone();
                let k = KeyString::from(key(style, n / 2) + "x");
                b.iter(|| {
                    m.insert(k.clone(), Value::Integer(1));
                    black_box(m.remove(k.as_str()));
                });
            });
            g.bench_with_input(BenchmarkId::new("remove_all_random", n), &n, |b, _| {
                b.iter_batched(
                    || map.clone(),
                    |mut m| {
                        for k in &lookup_order {
                            black_box(m.remove(k.as_str()));
                        }
                        m
                    },
                    BatchSize::SmallInput,
                );
            });
            g.bench_with_input(BenchmarkId::new("extend_half_overlap", n), &n, |b, _| {
                // Every other key overlaps; the rest interleave with existing keys.
                let other: Vec<(KeyString, Value)> = (0..n)
                    .map(|i| {
                        (
                            KeyString::from(key(style, i * 2 + (i % 2))),
                            Value::Integer(7),
                        )
                    })
                    .collect();
                b.iter_batched(
                    || (map.clone(), other.clone()),
                    |(mut m, o)| {
                        m.extend(o);
                        m
                    },
                    BatchSize::SmallInput,
                );
            });
            g.bench_with_input(BenchmarkId::new("iter", n), &n, |b, _| {
                b.iter(|| {
                    let mut total = 0usize;
                    for (k, v) in &map {
                        total += k.len() + usize::from(v.is_integer());
                    }
                    black_box(total)
                });
            });
            g.bench_with_input(BenchmarkId::new("clone", n), &n, |b, _| {
                b.iter(|| black_box(map.clone()));
            });
            g.bench_with_input(BenchmarkId::new("drop", n), &n, |b, _| {
                b.iter_batched(|| map.clone(), drop, BatchSize::SmallInput);
            });
            g.bench_with_input(BenchmarkId::new("eq", n), &n, |b, _| {
                let other = map.clone();
                b.iter(|| black_box(map == other));
            });
            g.bench_with_input(BenchmarkId::new("json_encode", n), &n, |b, _| {
                b.iter(|| black_box(serde_json::to_vec(&map).unwrap()));
            });
            g.bench_with_input(BenchmarkId::new("json_decode", n), &n, |b, _| {
                b.iter(|| black_box(serde_json::from_str::<Value>(&json).unwrap()));
            });
        }
        g.finish();
    }
}

fn compile_vrl(source: &str) -> Program {
    compile(source, &vrl::stdlib::all())
        .unwrap_or_else(|err| panic!("failed to compile VRL: {err:?}"))
        .program
}

fn target(value: Value) -> TargetValue {
    TargetValue {
        value,
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

/// `n` leaves spread over 2 levels of nesting: `group_XX.field_YYYY`.
fn nested(n: usize) -> Value {
    let groups = (n / 16).max(1);
    let mut root = ObjectMap::new();
    for (pos, i) in shuffled(n, 3).into_iter().enumerate() {
        let g = KeyString::from(format!("group_{:02}", i % groups));
        let entry = root
            .entry(g)
            .or_insert_with(|| Value::Object(ObjectMap::new()));
        if let Value::Object(child) = entry {
            child.insert(KeyString::from(format!("field_{i:04}")), value(pos));
        }
    }
    Value::Object(root)
}

fn flat(n: usize, style: KeyStyle) -> ObjectMap {
    build(&keys(style, n, 4))
}

fn run(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    runtime: &mut Runtime,
    name: &str,
    n: usize,
    program: &Program,
    input: &Value,
) {
    let tz = TimeZone::default();
    g.bench_with_input(BenchmarkId::new(name, n), &n, |b, _| {
        b.iter_batched(
            || target(input.clone()),
            |mut t| {
                black_box(runtime.resolve(&mut t, program, &tz).unwrap());
                t
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_vrl(c: &mut Criterion) {
    let mut g = c.benchmark_group("large_vrl");
    let tz = TimeZone::default();
    let mut runtime = Runtime::default();

    let assign_many = compile_vrl(
        &(0..10)
            .map(|i| format!(".new_{i:02} = {i}\n"))
            .collect::<String>(),
    );
    let read_many = compile_vrl(
        &(0..10)
            .map(|i| format!("x{i} = .field_{:04}\n", i * 3))
            .collect::<String>(),
    );
    let del_many = compile_vrl(
        &(0..10)
            .map(|i| format!("del(.field_{:04})\n", i * 3))
            .collect::<String>(),
    );
    let flatten = compile_vrl(". = flatten(.)");
    let unflatten = compile_vrl(". = unflatten(.)");
    let merge = compile_vrl(". = merge!(.base, .patch)");
    let merge_deep = compile_vrl(". = merge!(.base, .patch, deep: true)");
    let parse_json = compile_vrl(". = parse_json!(.message)");
    let map_values = compile_vrl(". = map_values(.) -> |v| { v }");
    let map_keys = compile_vrl(". = map_keys(.) -> |k| { k }");
    let compact = compile_vrl(". = compact(.)");
    let assign_100 = compile_vrl(
        &(0..100)
            .map(|i| format!(".new_{i:03} = {i}\n"))
            .collect::<String>(),
    );
    let parse_kv = compile_vrl(". = parse_key_value!(.message)");

    for &n in &VRL_SIZES {
        let flat_short = Value::Object(flat(n, KeyStyle::Short));
        run(
            &mut g,
            &mut runtime,
            "assign_10",
            n,
            &assign_many,
            &flat_short,
        );
        run(&mut g, &mut runtime, "read_10", n, &read_many, &flat_short);
        run(&mut g, &mut runtime, "del_10", n, &del_many, &flat_short);
        run(
            &mut g,
            &mut runtime,
            "map_values",
            n,
            &map_values,
            &flat_short,
        );
        run(&mut g, &mut runtime, "map_keys", n, &map_keys, &flat_short);
        run(&mut g, &mut runtime, "compact", n, &compact, &flat_short);

        let nested_v = nested(n);
        run(&mut g, &mut runtime, "flatten", n, &flatten, &nested_v);
        let flattened = {
            let mut t = target(nested_v.clone());
            runtime.resolve(&mut t, &flatten, &tz).unwrap();
            t.value
        };
        run(&mut g, &mut runtime, "unflatten", n, &unflatten, &flattened);

        // Two maps with interleaved keys: half of the patch keys overwrite base
        // keys, and half are new keys that sort between them.
        let base = flat(n, KeyStyle::Short);
        let patch: ObjectMap = (0..n / 2)
            .map(|i| {
                let existing = key(KeyStyle::Short, i * 2);
                let key = if i % 2 == 0 {
                    existing
                } else {
                    format!("{existing}_new")
                };
                (KeyString::from(key), Value::Integer(7))
            })
            .collect();
        let mut root = ObjectMap::new();
        root.insert("base".into(), Value::Object(base));
        root.insert("patch".into(), Value::Object(patch));
        let merge_input = Value::Object(root);
        run(&mut g, &mut runtime, "merge", n, &merge, &merge_input);
        run(
            &mut g,
            &mut runtime,
            "merge_deep",
            n,
            &merge_deep,
            &merge_input,
        );

        run(
            &mut g,
            &mut runtime,
            "assign_100",
            n,
            &assign_100,
            &flat_short,
        );
        let kv = shuffled(n, 5)
            .into_iter()
            .map(|i| format!("key_{i:04}=value{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mut root = ObjectMap::new();
        root.insert("message".into(), Value::from(kv));
        run(
            &mut g,
            &mut runtime,
            "parse_key_value",
            n,
            &parse_kv,
            &Value::Object(root),
        );

        let json = serde_json::to_string(&flat_short).unwrap();
        let mut root = ObjectMap::new();
        root.insert("message".into(), Value::from(json));
        run(
            &mut g,
            &mut runtime,
            "parse_json",
            n,
            &parse_json,
            &Value::Object(root),
        );
    }
    g.finish();
}

/// End-to-end large-event proxy: decode a JSON event, modify a few fields in
/// VRL (10 assignments, 10 deletions, 10 reads), encode it, drop it.
fn bench_pipeline(c: &mut Criterion) {
    let mut g = c.benchmark_group("large_pipeline");
    let tz = TimeZone::default();
    let mut runtime = Runtime::default();
    for (style, style_name) in [(KeyStyle::Short, "short"), (KeyStyle::Long, "long")] {
        // Quoted paths, since long keys contain dots.
        let mut source = String::new();
        for i in 0..10 {
            source += &format!(".new_{i:02} = {i}\n");
            source += &format!("del(.{:?})\n", key(style, i * 3));
            source += &format!("x{i} = .{:?}\n", key(style, i * 3 + 1));
        }
        let program = compile_vrl(&source);
        for &n in &PIPELINE_SIZES {
            let json = serde_json::to_vec(&flat(n, style)).unwrap();
            g.bench_with_input(
                BenchmarkId::new(format!("decode_remap_encode_{style_name}"), n),
                &n,
                |b, _| {
                    b.iter(|| {
                        let value: Value = serde_json::from_slice(&json).unwrap();
                        let mut t = target(value);
                        runtime.resolve(&mut t, &program, &tz).unwrap();
                        black_box(serde_json::to_vec(&t.value).unwrap())
                    });
                },
            );
        }
    }
    g.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(300))
        .measurement_time(std::time::Duration::from_millis(1200))
        .sample_size(20)
        .noise_threshold(0.03);
    targets = bench_map_ops, bench_vrl, bench_pipeline
);
criterion_main!(benches);
