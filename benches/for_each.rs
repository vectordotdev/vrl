use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::compiler::state::{ExternalEnv, RuntimeState, TypeState};
use vrl::compiler::{CompileConfig, Context, TargetValue, TimeZone, compile_with_state};
use vrl::value::kind::Collection;
use vrl::value::{KeyString, Kind, ObjectMap, Secrets, Value};

fn bench_for_each(c: &mut Criterion) {
    let mut group = c.benchmark_group("for_each");

    for size in [10, 1_000, 10_000] {
        // Array benchmark
        let array_val: Value = (0..size)
            .map(|i| Value::from(i as i64))
            .collect::<Vec<_>>()
            .into();
        let array_program = "sum = 0\nfor_each(.) -> |index, val| { sum = sum + val }";
        let fns = vrl::stdlib::all();
        let array_state = TypeState {
            local: Default::default(),
            external: ExternalEnv::new_with_kind(
                Kind::array(Collection::from_unknown(Kind::integer())),
                Kind::object(Collection::any()),
            ),
        };
        let res = compile_with_state(array_program, &fns, &array_state, CompileConfig::default())
            .expect("compiles");

        group.bench_with_input(BenchmarkId::new("array/bind_both", size), &size, |b, _| {
            b.iter(|| {
                let mut state = RuntimeState::default();
                let mut target = TargetValue {
                    value: array_val.clone(),
                    metadata: Value::Object(ObjectMap::new()),
                    secrets: Secrets::new(),
                };
                let timezone = TimeZone::default();
                let mut ctx = Context::new(&mut target, &mut state, &timezone);
                let _ = black_box(res.program.resolve(&mut ctx));
            });
        });

        // Wildcard array benchmark
        let wildcard_program = "sum = 0\nfor_each(.) -> |_, _| { sum = sum + 1 }";
        let res_wildcard = compile_with_state(
            wildcard_program,
            &fns,
            &array_state,
            CompileConfig::default(),
        )
        .expect("compiles");
        group.bench_with_input(BenchmarkId::new("array/wildcards", size), &size, |b, _| {
            b.iter(|| {
                let mut state = RuntimeState::default();
                let mut target = TargetValue {
                    value: array_val.clone(),
                    metadata: Value::Object(ObjectMap::new()),
                    secrets: Secrets::new(),
                };
                let timezone = TimeZone::default();
                let mut ctx = Context::new(&mut target, &mut state, &timezone);
                let _ = black_box(res_wildcard.program.resolve(&mut ctx));
            });
        });

        // Object benchmark
        let mut map = ObjectMap::new();
        for i in 0..size {
            map.insert(KeyString::from(format!("key_{i}")), Value::from(i as i64));
        }
        let object_val = Value::Object(map);
        let object_program = "sum = 0\nfor_each(.) -> |key, val| { sum = sum + val }";
        let object_state = TypeState {
            local: Default::default(),
            external: ExternalEnv::new_with_kind(
                Kind::object(Collection::from_unknown(Kind::integer())),
                Kind::object(Collection::any()),
            ),
        };
        let res_obj = compile_with_state(
            object_program,
            &fns,
            &object_state,
            CompileConfig::default(),
        )
        .expect("compiles");
        group.bench_with_input(BenchmarkId::new("object/bind_both", size), &size, |b, _| {
            b.iter(|| {
                let mut state = RuntimeState::default();
                let mut target = TargetValue {
                    value: object_val.clone(),
                    metadata: Value::Object(ObjectMap::new()),
                    secrets: Secrets::new(),
                };
                let timezone = TimeZone::default();
                let mut ctx = Context::new(&mut target, &mut state, &timezone);
                let _ = black_box(res_obj.program.resolve(&mut ctx));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_for_each);
criterion_main!(benches);
