use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::json;
use vrl::{path, path::BorrowedValuePath, value::Value};

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

fn benchmark_value_insert_sequence(c: &mut Criterion) {
    let service_paths = [
        path!("service", "name"),
        path!("service", "version"),
        path!("service", "environment"),
    ];
    let metadata_paths = [
        path!("service", "metadata", "name"),
        path!("service", "metadata", "version"),
        path!("service", "metadata", "environment"),
    ];
    let service = json!({"name": "api", "version": "1.0", "environment": "production"});
    let payloads = ["api", "1.0", "production"].map(Value::from);
    let cases = [
        (
            "missing_parent",
            Value::from(json!({"message": "hello"})),
            service_paths,
            Value::from(json!({"message": "hello", "service": service.clone()})),
        ),
        (
            "existing_parent",
            Value::from(json!({"message": "hello", "service": {}})),
            service_paths,
            Value::from(json!({"message": "hello", "service": service.clone()})),
        ),
        (
            "partially_existing_path",
            Value::from(json!({"message": "hello", "service": {"owner": "payments"}})),
            metadata_paths,
            Value::from(json!({
                "message": "hello",
                "service": {"owner": "payments", "metadata": service},
            })),
        ),
    ];

    // Validate the entire sequence outside timing, including preservation of
    // the message and unrelated service fields.
    for (case, fixture, paths, expected) in &cases {
        let mut value = fixture.clone();
        insert_sequence(&mut value, paths, &payloads);
        assert_eq!(&value, expected, "{case}");
    }

    for (name, batch_size) in [
        ("value_insert_sequence", Some(BatchSize::SmallInput)),
        (
            "value_insert_sequence_bounded_64",
            Some(BatchSize::NumIterations(64)),
        ),
        ("value_insert_sequence_full_lifetime", None),
    ] {
        let mut group = c.benchmark_group(name);
        group.throughput(Throughput::Elements(3));
        for (case, fixture, paths, _) in &cases {
            group.bench_function(*case, |b| {
                if let Some(batch_size) = batch_size {
                    // Each event starts fresh. When a parent is missing, the
                    // first assignment creates it and the next two reuse it.
                    b.iter_batched_ref(
                        || fixture.clone(),
                        |value| insert_sequence(value, paths, &payloads),
                        batch_size,
                    );
                } else {
                    // Includes fixture cloning and destruction, as in the
                    // single-insert full-lifetime controls above.
                    b.iter(|| {
                        let mut value = black_box(fixture.clone());
                        insert_sequence(&mut value, paths, &payloads);
                        drop(black_box(value));
                    });
                }
            });
        }
        group.finish();
    }
}

fn insert_sequence(
    value: &mut Value,
    paths: &[BorrowedValuePath<'_, '_>; 3],
    payloads: &[Value; 3],
) {
    // Payloads are prepared once; cloning their values remains part of timing.
    // An iteration measures all three assignments, not just the final insert.
    for (path, payload) in paths.iter().zip(payloads) {
        black_box(value.insert(black_box(*path), black_box(payload.clone())));
    }
}

criterion_group!(
    benches,
    benchmark_value_insert,
    benchmark_value_insert_sequence
);
criterion_main!(benches);
