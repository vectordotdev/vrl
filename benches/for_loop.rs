use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::compiler::state::{ExternalEnv, LocalEnv, RuntimeState, TypeState};
use vrl::compiler::{CompileConfig, Context, TargetValue, TimeZone, compile_with_state};
use vrl::value::kind::{Collection, Field};
use vrl::value::{KeyString, Kind, ObjectMap, Secrets, Value};

fn bench_array_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration/array");

    for size in [100, 1_000, 10_000] {
        let array_val: Value = (0..size)
            .map(|i| Value::from(i64::from(i)))
            .collect::<Vec<_>>()
            .into();

        let mut map = ObjectMap::new();
        map.insert(KeyString::from("arr"), array_val);
        let target_val = Value::Object(map);

        let for_loop_src = "sum = 0\nfor v in .arr { sum = sum + v }";
        let for_each_src = "sum = 0\nfor_each(.arr) -> |_, v| { sum = sum + v }";

        let mut collection_map = BTreeMap::new();
        collection_map.insert(
            Field::from("arr"),
            Kind::array(Collection::from_unknown(Kind::integer())),
        );
        let array_state = TypeState {
            local: LocalEnv::default(),
            external: ExternalEnv::new_with_kind(
                Kind::object(collection_map),
                Kind::object(Collection::any()),
            ),
        };

        let fns = vrl::stdlib::all();
        let res_for =
            compile_with_state(for_loop_src, &fns, &array_state, CompileConfig::default())
                .expect("compiles for loop");
        let res_for_each =
            compile_with_state(for_each_src, &fns, &array_state, CompileConfig::default())
                .expect("compiles for_each");

        group.bench_with_input(BenchmarkId::new("for_loop", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("for_each", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for_each.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_object_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration/object");

    for size in [100, 1_000] {
        let mut obj = ObjectMap::new();
        for i in 0..size {
            obj.insert(
                KeyString::from(format!("key_{i}")),
                Value::from(i64::from(i)),
            );
        }

        let mut map = ObjectMap::new();
        map.insert(KeyString::from("obj"), Value::Object(obj));
        let target_val = Value::Object(map);

        let for_loop_src = "sum = 0\nfor k, v in .obj { sum = sum + v }";
        let for_each_src = "sum = 0\nfor_each(.obj) -> |k, v| { sum = sum + v }";

        let mut collection_map = BTreeMap::new();
        collection_map.insert(
            Field::from("obj"),
            Kind::object(Collection::from_unknown(Kind::integer())),
        );
        let object_state = TypeState {
            local: LocalEnv::default(),
            external: ExternalEnv::new_with_kind(
                Kind::object(collection_map),
                Kind::object(Collection::any()),
            ),
        };

        let fns = vrl::stdlib::all();
        let res_for =
            compile_with_state(for_loop_src, &fns, &object_state, CompileConfig::default())
                .expect("compiles for loop");
        let res_for_each =
            compile_with_state(for_each_src, &fns, &object_state, CompileConfig::default())
                .expect("compiles for_each");

        group.bench_with_input(BenchmarkId::new("for_loop", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("for_each", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for_each.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_break_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("iteration/break");

    for size in [100, 1_000, 10_000] {
        let array_val: Value = (0..size)
            .map(|i| Value::from(i64::from(i)))
            .collect::<Vec<_>>()
            .into();

        let mut map = ObjectMap::new();
        map.insert(KeyString::from("arr"), array_val);
        let target_val = Value::Object(map);

        // First-class for loop with native `break` at index 50.
        let for_loop_src = "found = null\nfor v in .arr {\n    if v == 50 {\n        found = v\n        break\n    }\n}";
        // `for_each` closure early-exit simulation using `break` to measure overhead vs native loop.
        let for_each_src = "found = null\nfor_each(.arr) -> |_, v| {\n    if v == 50 {\n        found = v\n        break\n    }\n}";

        let mut collection_map = BTreeMap::new();
        collection_map.insert(
            Field::from("arr"),
            Kind::array(Collection::from_unknown(Kind::integer())),
        );
        let array_state = TypeState {
            local: LocalEnv::default(),
            external: ExternalEnv::new_with_kind(
                Kind::object(collection_map),
                Kind::object(Collection::any()),
            ),
        };

        let fns = vrl::stdlib::all();
        let res_for =
            compile_with_state(for_loop_src, &fns, &array_state, CompileConfig::default())
                .expect("compiles for loop with break");
        let res_for_each =
            compile_with_state(for_each_src, &fns, &array_state, CompileConfig::default())
                .expect("compiles for_each with early exit");

        group.bench_with_input(BenchmarkId::new("for_loop", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("for_each", size), &size, |b, _| {
            b.iter_batched(
                || {
                    (
                        RuntimeState::default(),
                        TargetValue {
                            value: target_val.clone(),
                            metadata: Value::Object(ObjectMap::new()),
                            secrets: Secrets::new(),
                        },
                    )
                },
                |(mut state, mut target)| {
                    let timezone = TimeZone::default();
                    let mut ctx = Context::new(&mut target, &mut state, &timezone);
                    let _ = black_box(res_for_each.program.resolve(&mut ctx));
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_array_iteration,
    bench_object_iteration,
    bench_break_iteration,
);
criterion_main!(benches);
