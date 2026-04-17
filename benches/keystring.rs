use std::borrow::Cow;
use std::hint::black_box;

use bytes::Bytes;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

use vrl::compiler::runtime::Runtime;
use vrl::compiler::{Program, TargetValue, TimeZone, compile};
use vrl::path::parse_value_path;
use vrl::value::{KeyString, ObjectMap, Secrets, Value};

// ---------------------------------------------------------------------------
// Bench 1: keystring_micro
// ---------------------------------------------------------------------------

fn bench_keystring_micro(c: &mut Criterion) {
    let mut group = c.benchmark_group("keystring_micro");

    // from_short_str: From<&str> for an 8-byte string (inline for SSO types)
    group.bench_function("from_short_str", |b| {
        b.iter(|| {
            black_box(KeyString::from("hostname"));
        });
    });

    // from_medium_str: From<&str> for a 22-byte string
    // Inline for CompactString (24B), spills for EcoString (15B)
    group.bench_function("from_medium_str", |b| {
        b.iter(|| {
            black_box(KeyString::from("upstream_response_time"));
        });
    });

    // from_long_str: From<&str> for a 32-byte string (spills all SSO types)
    group.bench_function("from_long_str", |b| {
        b.iter(|| {
            black_box(KeyString::from("x_upstream_forwarded_for_header"));
        });
    });

    // from_string_short: From<String>
    group.bench_function("from_string_short", |b| {
        b.iter_batched(
            || String::from("hostname"),
            |s| {
                black_box(KeyString::from(s));
            },
            BatchSize::SmallInput,
        );
    });

    // from_string_medium: From<String> — CompactString can take ownership, EcoString copies
    group.bench_function("from_string_medium", |b| {
        b.iter_batched(
            || String::from("upstream_response_time"),
            |s| {
                black_box(KeyString::from(s));
            },
            BatchSize::SmallInput,
        );
    });

    // from_string_long: From<String> for a spilled key
    group.bench_function("from_string_long", |b| {
        b.iter_batched(
            || String::from("x_upstream_forwarded_for_header"),
            |s| {
                black_box(KeyString::from(s));
            },
            BatchSize::SmallInput,
        );
    });

    // from_cow_borrowed: the path traversal conversion
    group.bench_function("from_cow_borrowed", |b| {
        b.iter(|| {
            let cow: Cow<'_, str> = Cow::Borrowed("hostname");
            black_box(KeyString::from(cow));
        });
    });

    // clone_short: inline for all SSO types
    let short_key = KeyString::from("hostname");
    group.bench_function("clone_short", |b| {
        b.iter(|| {
            black_box(short_key.clone());
        });
    });

    // clone_medium: inline for CompactString, spilled for EcoString
    let medium_key = KeyString::from("upstream_response_time");
    group.bench_function("clone_medium", |b| {
        b.iter(|| {
            black_box(medium_key.clone());
        });
    });

    // clone_long: spilled for all SSO types — fair heap comparison
    let long_key = KeyString::from("x_upstream_forwarded_for_header");
    group.bench_function("clone_long", |b| {
        b.iter(|| {
            black_box(long_key.clone());
        });
    });

    // roundtrip: KeyString → .as_str() → KeyString::from(s)
    // This is the OwnedSegment → BorrowedSegment → KeyString path
    let key = KeyString::from("hostname");
    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let s = key.as_str();
            black_box(KeyString::from(s));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Bench 2: path_ops
// ---------------------------------------------------------------------------

/// Build a realistic 15-field event resembling a Datadog log.
fn build_event() -> Value {
    let mut map = ObjectMap::new();
    let fields = [
        ("message", "2024-01-15T10:30:00Z INFO server started successfully on port 8080"),
        ("hostname", "web-prod-01.us-east-1.compute.internal"),
        ("status", "info"),
        ("severity", "informational"),
        ("facility", "local0"),
        ("appname", "vector"),
        ("timestamp", "2024-01-15T10:30:00.123456Z"),
        ("procid", "12345"),
        ("source_type", "datadog_agent"),
        ("service", "vector-aggregator"),
        ("env", "production"),
        ("version", "0.34.0"),
        ("trace_id", "4bf92f3577b34da6a3ce929d0e0e4736"),
        ("span_id", "00f067aa0ba902b7"),
        ("tags", "env:prod,service:vector,version:0.34.0"),
    ];
    for (k, v) in fields {
        map.insert(k.into(), Value::Bytes(Bytes::from(v)));
    }
    Value::Object(map)
}

/// Build a nested event with a `foo.bar.baz` path.
fn build_nested_event() -> Value {
    let mut root = ObjectMap::new();

    // Add the 15 fields from the flat event
    let fields = [
        ("message", "test message"),
        ("hostname", "web-prod-01"),
        ("status", "info"),
        ("severity", "informational"),
        ("facility", "local0"),
        ("appname", "vector"),
        ("timestamp", "2024-01-15T10:30:00Z"),
        ("procid", "12345"),
        ("source_type", "datadog_agent"),
        ("service", "vector-aggregator"),
        ("env", "production"),
        ("version", "0.34.0"),
        ("trace_id", "4bf92f3577b34da6"),
        ("span_id", "00f067aa0ba902b7"),
        ("tags", "env:prod"),
    ];
    for (k, v) in fields {
        root.insert(k.into(), Value::Bytes(Bytes::from(v)));
    }

    // Add nested structure: foo.bar.baz = "nested_value"
    let mut baz_map = ObjectMap::new();
    baz_map.insert("baz".into(), Value::Bytes(Bytes::from("nested_value")));
    let mut bar_map = ObjectMap::new();
    bar_map.insert("bar".into(), Value::Object(baz_map));
    root.insert("foo".into(), Value::Object(bar_map));

    Value::Object(root)
}

fn bench_path_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_ops");

    let event = build_event();
    let nested_event = build_nested_event();

    // Pre-compile owned paths
    let owned_path = parse_value_path("service").unwrap();
    let owned_nested_path = parse_value_path("foo.bar.baz").unwrap();

    // owned_path_get: compiled path lookup
    group.bench_function("owned_path_get", |b| {
        b.iter(|| {
            black_box(event.get(&owned_path));
        });
    });

    // owned_path_insert: compiled path insert (includes KeyString roundtrip)
    group.bench_function("owned_path_insert", |b| {
        b.iter_batched(
            || event.clone(),
            |mut value| {
                black_box(value.insert(&owned_path, Value::Bytes(Bytes::from_static(b"new"))));
            },
            BatchSize::SmallInput,
        );
    });

    // owned_path_nested_get: 3-level compiled path lookup
    group.bench_function("owned_path_nested_get", |b| {
        b.iter(|| {
            black_box(nested_event.get(&owned_nested_path));
        });
    });

    // owned_path_nested_insert: 3-level insert (multiple KeyString reconstructions)
    group.bench_function("owned_path_nested_insert", |b| {
        b.iter_batched(
            || nested_event.clone(),
            |mut value| {
                black_box(value.insert(
                    &owned_nested_path,
                    Value::Bytes(Bytes::from_static(b"new")),
                ));
            },
            BatchSize::SmallInput,
        );
    });

    // jit_path_get: JIT string path lookup
    group.bench_function("jit_path_get", |b| {
        b.iter(|| {
            black_box(event.get("service"));
        });
    });

    // jit_path_insert: JIT string path insert
    group.bench_function("jit_path_insert", |b| {
        b.iter_batched(
            || event.clone(),
            |mut value| {
                black_box(value.insert("service", Value::Bytes(Bytes::from_static(b"new"))));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Bench 3: vrl_programs
// ---------------------------------------------------------------------------

/// Helper to compile a VRL program with all stdlib functions.
fn compile_vrl(source: &str) -> Program {
    let fns = vrl::stdlib::all();
    compile(source, &fns)
        .unwrap_or_else(|err| panic!("failed to compile VRL: {err:?}"))
        .program
}

/// Build a target suitable for the remap_fields benchmark.
fn build_remap_target() -> TargetValue {
    let mut map = ObjectMap::new();
    let fields = [
        ("message", "2024-01-15T10:30:00Z INFO server started"),
        ("hostname", "web-prod-01"),
        ("status", "warning"),
        ("severity", "informational"),
        ("facility", "local0"),
        ("appname", "vector"),
        ("timestamp", "2024-01-15T10:30:00Z"),
        ("procid", "12345"),
        ("source_type", "datadog_agent"),
        ("service", "vector-aggregator"),
        ("env", "production"),
        ("version", "0.34.0"),
        ("trace_id", "4bf92f3577b34da6a3ce929d0e0e4736"),
        ("span_id", "00f067aa0ba902b7"),
        ("tags", "env:prod,service:vector,version:0.34.0"),
    ];
    for (k, v) in fields {
        map.insert(k.into(), Value::Bytes(Bytes::from(v)));
    }
    TargetValue {
        value: Value::Object(map),
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

/// Build a target with a valid syslog message.
fn build_syslog_target() -> TargetValue {
    let mut map = ObjectMap::new();
    map.insert(
        "message".into(),
        Value::Bytes(Bytes::from(
            "<165>1 2024-01-15T10:30:00.123456Z web-prod-01 vector 12345 ID47 \
             [exampleSDID@32473 iut=\"3\" eventSource=\"Application\" \
             eventID=\"1011\"] An application event log entry",
        )),
    );
    TargetValue {
        value: Value::Object(map),
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

/// Build a target for object construction benchmark.
fn build_object_construction_target() -> TargetValue {
    let mut map = ObjectMap::new();
    let fields = [
        ("method", "GET"),
        ("path", "/api/v1/events"),
        ("status", "200"),
        ("duration", "42"),
        ("host", "web-prod-01"),
        ("service", "vector-aggregator"),
        ("env", "production"),
        ("version", "0.34.0"),
        ("trace_id", "4bf92f3577b34da6a3ce929d0e0e4736"),
        ("message", "handled request successfully"),
    ];
    for (k, v) in fields {
        map.insert(k.into(), Value::Bytes(Bytes::from(v)));
    }
    TargetValue {
        value: Value::Object(map),
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

fn bench_vrl_programs(c: &mut Criterion) {
    let mut group = c.benchmark_group("vrl_programs");
    let tz = TimeZone::default();
    let mut runtime = Runtime::default();

    // remap_fields: correlation with Vector regression benchmark (lookup-dominated)
    let remap_program = compile_vrl(
        r#"
        .hostname = "vector"
        if .status == "warning" {
            .thing = upcase!(.hostname)
        }
        .new_field = "value"
        .service = downcase!(.service)
        .env_upper = upcase!(.env)
        .has_trace = exists(.trace_id)
        "#,
    );

    group.bench_function("remap_fields", |b| {
        b.iter_batched(
            || build_remap_target(),
            |mut target| {
                black_box(runtime.resolve(&mut target, &remap_program, &tz).unwrap());
            },
            BatchSize::SmallInput,
        );
    });

    // parse_syslog: stdlib object construction (~9-key ObjectMap)
    let syslog_program = compile_vrl(". = parse_syslog!(.message)");

    group.bench_function("parse_syslog", |b| {
        b.iter_batched(
            || build_syslog_target(),
            |mut target| {
                black_box(runtime.resolve(&mut target, &syslog_program, &tz).unwrap());
            },
            BatchSize::SmallInput,
        );
    });

    // parse_and_flatten: object construction + compound key construction
    let flatten_program = compile_vrl("parsed = parse_syslog!(.message)\n. = flatten(parsed)");

    group.bench_function("parse_and_flatten", |b| {
        b.iter_batched(
            || build_syslog_target(),
            |mut target| {
                black_box(runtime.resolve(&mut target, &flatten_program, &tz).unwrap());
            },
            BatchSize::SmallInput,
        );
    });

    // object_construction: 10-field object literal (exercises Object::resolve cloning KeyStrings)
    let obj_program = compile_vrl(
        r#"
        .result = {
            "method": .method,
            "path": .path,
            "status": .status,
            "duration": .duration,
            "host": .host,
            "service": .service,
            "env": .env,
            "version": .version,
            "trace_id": .trace_id,
            "message": .message
        }
        "#,
    );

    group.bench_function("object_construction", |b| {
        b.iter_batched(
            || build_object_construction_target(),
            |mut target| {
                black_box(runtime.resolve(&mut target, &obj_program, &tz).unwrap());
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Bench 4: json_deser
// ---------------------------------------------------------------------------

const JSON_EVENT: &str = r#"{
    "host": "web-prod-01",
    "env": "production",
    "service": "vector-aggregator",
    "version": "0.34.0",
    "message": "handled request successfully with detailed tracing information",
    "hostname": "web-prod-01.us-east-1.compute.internal",
    "severity": "informational",
    "facility": "local0",
    "appname": "vector",
    "procid": "12345",
    "source_type": "datadog_agent",
    "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
    "span_id": "00f067aa0ba902b7",
    "tags": "env:prod,service:vector,version:0.34.0",
    "upstream_response_time": "0.042",
    "http": {
        "method": "GET",
        "status": 200,
        "path": "/api/v1/events",
        "response_bytes": 1024
    }
}"#;

fn bench_json_deser(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deser");

    // via_serde_json_value: the actual Vector path (String → KeyString)
    group.bench_function("via_serde_json_value", |b| {
        b.iter(|| {
            let json_val: serde_json::Value = serde_json::from_str(JSON_EVENT).unwrap();
            black_box(Value::from(json_val));
        });
    });

    // direct_deserialize: direct to Value (KeyString's Deserialize → visit_str, no String intermediate)
    group.bench_function("direct_deserialize", |b| {
        b.iter(|| {
            black_box(serde_json::from_str::<Value>(JSON_EVENT).unwrap());
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    name = benches;
    config = Criterion::default().noise_threshold(0.05);
    targets =
        bench_keystring_micro,
        bench_path_ops,
        bench_vrl_programs,
        bench_json_deser
);
criterion_main!(benches);
