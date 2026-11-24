//! Benchmarks for object literal resolution: structurally-constant lookup
//! tables resolve from the compile-time cache, mixed objects (one dynamic
//! entry) take the per-entry path. Run on both sides of the cache change
//! to compare.
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::btreemap;
use vrl::compiler::runtime::Runtime;
use vrl::compiler::{Program, TargetValue, TimeZone, compile};
use vrl::value::{Secrets, Value};

const SIZES: [usize; 4] = [8, 32, 128, 512];

/// A program assigning an object literal of `size` entries, optionally with
/// one dynamic entry, then returning it — the remap lookup-table pattern.
fn lookup_table_program(size: usize, dynamic: bool) -> String {
    let mut entries = (0..size)
        .map(|i| format!(r#""key_{i:04}": {i}"#))
        .collect::<Vec<_>>();
    if dynamic {
        entries.push(r#""dyn": to_int(.field) ?? 0"#.to_owned());
    }
    format!("tbl = {{ {} }}\ntbl", entries.join(", "))
}

fn compile_program(source: &str) -> Program {
    compile(source, &vrl::stdlib::all())
        .unwrap_or_else(|err| panic!("failed to compile VRL: {err:?}"))
        .program
}

fn bench_resolve(c: &mut Criterion) {
    let mut group = c.benchmark_group("object_constant");
    for &size in SIZES.iter() {
        for (name, dynamic) in [("constant", false), ("mixed", true)] {
            let program = compile_program(&lookup_table_program(size, dynamic));
            let mut runtime = Runtime::default();
            group.bench_with_input(BenchmarkId::new(name, size), &(), |b, _| {
                b.iter_batched(
                    || TargetValue {
                        value: Value::from(btreemap! { "field" => 42 }),
                        metadata: Value::from(btreemap! {}),
                        secrets: Secrets::new(),
                    },
                    |mut target| {
                        black_box(
                            runtime
                                .resolve(&mut target, &program, &TimeZone::default())
                                .unwrap(),
                        );
                    },
                    BatchSize::SmallInput,
                )
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_resolve);
criterion_main!(benches);
