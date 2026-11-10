use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use vrl::{path, value::Value};

fn benchmark_value_insert(c: &mut Criterion) {
    for (name, batch_size) in [
        ("value_insert", Some(BatchSize::SmallInput)),
        (
            "value_insert_bounded_64",
            Some(BatchSize::NumIterations(64)),
        ),
        ("value_insert_full_lifetime", None),
    ] {
        let mut group = c.benchmark_group(name);
        for size in [0, 8, 32, 128] {
            let labels = (0..size)
                .map(|i| (format!("label_{i:04}").into(), Value::Integer(i)))
                .collect();
            let mut existing_prefix = Value::Null;
            existing_prefix.insert(path!("kubernetes", "pod_labels"), Value::Object(labels));
            let mut existing_leaf = existing_prefix.clone();
            existing_leaf.insert(path!("kubernetes", "pod_labels", "new_label"), 0);

            for (case, fixture) in [
                ("new_leaf", &existing_prefix),
                ("overwrite", &existing_leaf),
                ("missing_prefix", &Value::Null),
            ] {
                group.bench_with_input(BenchmarkId::new(case, size), fixture, |b, fixture| {
                    if let Some(batch_size) = batch_size {
                        // Every insertion gets a fresh fixture, so new_leaf never becomes
                        // an overwrite. Clone and final drop are outside the timed routine.
                        // Bounded batches retain at most 64 resulting objects at a time.
                        b.iter_batched_ref(|| fixture.clone(), insert_leaf, batch_size);
                    } else {
                        // Time the whole fixture lifecycle, including clone and final drop.
                        // Populated fixtures therefore include cloning their existing labels;
                        // this is not an insertion-only measurement.
                        b.iter(|| {
                            let mut value = black_box(fixture.clone());
                            insert_leaf(&mut value);
                            drop(black_box(value));
                        });
                    }
                });
            }
        }
        group.finish();
    }
}

fn insert_leaf(value: &mut Value) {
    black_box(value.insert(
        black_box(path!("kubernetes", "pod_labels", "new_label")),
        black_box(Value::Integer(42)),
    ));
}

criterion_group!(benches, benchmark_value_insert);
criterion_main!(benches);
