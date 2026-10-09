use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::json;
use vrl::compiler::{TargetValue, TimeZone, compile, runtime::Runtime};
use vrl::value::{ObjectMap, Secrets, Value};

fn fresh_target(event: &Value) -> TargetValue {
    TargetValue {
        value: event.clone(),
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::new(),
    }
}

fn bench_program(
    c: &mut Criterion,
    name: &str,
    source: &str,
    event: &Value,
    expected_result: &Value,
    expected_target: &Value,
) {
    let program = compile(source, &vrl::stdlib::all())
        .expect("program compiles")
        .program;
    let timezone = TimeZone::default();
    let mut runtime = Runtime::default();
    let mut target = fresh_target(event);
    let result = runtime
        .resolve(&mut target, &program, &timezone)
        .expect("program resolves");
    assert_eq!(&result, expected_result);
    assert_eq!(&target.value, expected_target);
    runtime.clear();

    c.bench_function(name, |b| {
        b.iter_batched(
            || fresh_target(event),
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

fn bench_variable_reads(c: &mut Criterion) {
    let large = json!({
        "user": {"id": "u-123", "name": "alice", "role": "admin"},
        "event": {"kind": "login", "timestamp": "2026-01-01T00:00:00Z"},
        "message": "User logged in",
        "host": "web-1",
        "env": "prod",
        "region": "us-east-1",
        "service": "auth",
        "version": "2.3.1",
        "request_id": "req-abc",
        "status": 200,
        "latency": 42,
        "bytes_in": 512,
        "bytes_out": 1024,
        "cached": false,
        "tls": true,
        "protocol": "HTTP/2",
        "method": "POST",
        "path": "/api/v2/login",
        "user_agent": "curl/7.68",
        "tags": ["login", "production"]
    });
    let large_event = Value::from(json!({"payload": &large}));
    bench_program(
        c,
        "variable_reads/large_subpath",
        "parsed = .payload\n[parsed.user.id, parsed.event.kind, parsed.host, parsed.env, parsed.region]",
        &large_event,
        &Value::from(json!(["u-123", "login", "web-1", "prod", "us-east-1"])),
        &large_event,
    );

    let small_event = Value::from(json!({
        "payload": {"id": "u-123", "name": "alice", "role": "admin", "env": "prod", "region": "us-east-1"}
    }));
    bench_program(
        c,
        "variable_reads/small_subpath",
        "parsed = .payload\nparsed.id",
        &small_event,
        &Value::from("u-123"),
        &small_event,
    );

    // Whole-variable reads provide a control for changes to subpath lookup.
    bench_program(
        c,
        "variable_reads/whole_variable",
        "parsed = .payload\nparsed",
        &large_event,
        &Value::from(large.clone()),
        &large_event,
    );

    let remap_event = Value::from(json!({
        "message": serde_json::to_string(&large).expect("payload serializes")
    }));
    let remap_result = Value::from(json!({
        "message": "user logged in",
        "user": "alice",
        "service": "auth",
        "status": 200,
        "region": "us-east-1",
        "tags": ["login", "production"]
    }));
    bench_program(
        c,
        "variable_reads/parse_and_remap",
        r#"
parsed = parse_json!(.message)
. = {
    "message": downcase(string!(parsed.message)),
    "user": parsed.user.name,
    "service": parsed.service,
    "status": parsed.status,
    "region": parsed.region,
    "tags": parsed.tags
}
"#,
        &remap_event,
        &remap_result,
        &remap_result,
    );
}

criterion_group!(benches, bench_variable_reads);
criterion_main!(benches);
