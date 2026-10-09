use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::json;
use vrl::compiler::{TargetValue, TimeZone, compile, runtime::Runtime};
use vrl::value::{Secrets, Value};

fn fresh_target(event: &Value, metadata: &Value) -> TargetValue {
    TargetValue {
        value: event.clone(),
        metadata: metadata.clone(),
        secrets: Secrets::new(),
    }
}

fn bench_program(
    c: &mut Criterion,
    name: &str,
    source: &str,
    event: &Value,
    metadata: &Value,
    expected_result: &Value,
    expected_target: &Value,
) {
    let program = compile(source, &vrl::stdlib::all())
        .expect("program compiles")
        .program;
    let timezone = TimeZone::default();
    let mut runtime = Runtime::default();
    let mut target = fresh_target(event, metadata);
    let result = runtime
        .resolve(&mut target, &program, &timezone)
        .expect("program resolves");
    assert_eq!(&result, expected_result);
    assert_eq!(&target.value, expected_target);
    assert_eq!(&target.metadata, metadata);
    runtime.clear();

    c.bench_function(name, |b| {
        b.iter_batched(
            || fresh_target(event, metadata),
            |mut target| {
                runtime.clear();
                let result = runtime
                    .resolve(&mut target, black_box(&program), &timezone)
                    .expect("program resolves");
                black_box((result, target));
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_external_field_reads(c: &mut Criterion) {
    let empty = Value::from(json!({}));
    for (name, source, event) in [
        ("depth_1", ".a", json!({"a": 42})),
        ("depth_2", ".a.b", json!({"a": {"b": 42}})),
        ("depth_3", ".a.b.c", json!({"a": {"b": {"c": 42}}})),
    ] {
        let event = Value::from(event);
        bench_program(
            c,
            &format!("external_field_reads/{name}"),
            source,
            &event,
            &empty,
            &Value::from(42),
            &event,
        );
    }

    let metadata = Value::from(json!({"a": {"b": {"c": 42}}}));
    bench_program(
        c,
        "external_field_reads/metadata_depth_3",
        "%a.b.c",
        &empty,
        &metadata,
        &Value::from(42),
        &empty,
    );

    // Root reads have no path segments to allocate.
    bench_program(
        c,
        "external_field_reads/root",
        ".",
        &metadata,
        &empty,
        &metadata,
        &metadata,
    );

    let event = Value::from(json!({
        "message": "User logged in",
        "user": {"id": "u-123", "name": "alice"},
        "http": {"response": {"status_code": 200}},
        "service": "auth",
        "region": "us-east-1",
        "tags": ["login", "production"]
    }));
    let metadata = Value::from(json!({"datadog": {"source": "application"}}));
    let result = Value::from(json!({
        "message": "user logged in",
        "user": "alice",
        "user_id": "u-123",
        "status": 200,
        "service": "auth",
        "region": "us-east-1",
        "tags": ["login", "production"],
        "source": "application"
    }));
    bench_program(
        c,
        "external_field_reads/remap",
        r#"
. = {
    "message": downcase(string!(.message)),
    "user": .user.name,
    "user_id": .user.id,
    "status": .http.response.status_code,
    "service": .service,
    "region": .region,
    "tags": .tags,
    "source": %datadog.source
}
"#,
        &event,
        &metadata,
        &result,
        &result,
    );
}

criterion_group!(benches, bench_external_field_reads);
criterion_main!(benches);
