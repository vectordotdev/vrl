//! Benchmarks for operations sensitive to `ObjectMap`/`Value` memory layout:
//! event construction, clone, drop, lookup, JSON encode/decode, and small VRL
//! programs.
use std::hint::black_box;

use bytes::Bytes;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use vrl::compiler::runtime::Runtime;
use vrl::compiler::{Program, TargetValue, TimeZone, compile};
use vrl::value::{KeyString, ObjectMap, Secrets, Value};

/// Field shape of an event produced by the `datadog_agent` source.
const DD_FIELDS: [(&str, &str); 8] = [
    (
        "message",
        "2024-01-15T10:30:00Z INFO server started successfully on port 8080",
    ),
    ("status", "warning"),
    ("timestamp", "2024-01-15T10:30:00.123456Z"),
    ("hostname", "web-prod-01.us-east-1.compute.internal"),
    ("service", "vector-aggregator"),
    ("ddsource", "vector"),
    ("ddtags", "env:prod,service:vector,version:0.34.0"),
    ("source_type", "datadog_agent"),
];

const SYSLOG_LINE: &str = "<13>1 2020-03-13T20:45:38.119Z dynamicwireless.name non 2426 ID931 [exampleSDID@32473 iut=\"3\" eventSource= \"Application\" eventID=\"1011\"] Try to override the THX port, maybe it will reboot the neural interface!";

const JSON_EVENT: &str = r#"{"message":"GET /api/v1/users HTTP/1.1","host":"web-01","env":"prod","service":"api","status":200,"duration_ms":12.5,"timestamp":"2024-01-15T10:30:00Z","hostname":"web-prod-01.us-east-1","severity":"info","trace_id":"4bf92f3577b34da6a3ce929d0e0e4736","span_id":"00f067aa0ba902b7","upstream_response_time":"0.012","user_agent":"curl/8.0","http":{"method":"GET","status_code":200,"url":"/api/v1/users","version":"1.1"},"tags":["a","b","c"]}"#;

fn build_dd() -> Value {
    let mut map = ObjectMap::new();
    for (k, v) in DD_FIELDS {
        map.insert(
            KeyString::from(k),
            Value::Bytes(Bytes::from_static(v.as_bytes())),
        );
    }
    Value::Object(map)
}

fn big_keys(n: usize) -> Vec<KeyString> {
    // Deterministic, unsorted key order.
    (0..n)
        .map(|i| KeyString::from(format!("field_{:03}", (i * 37) % n)))
        .collect()
}

fn build_big(keys: &[KeyString]) -> Value {
    let mut map = ObjectMap::new();
    for k in keys {
        map.insert(k.clone(), Value::Integer(1));
    }
    Value::Object(map)
}

fn compile_vrl(source: &str) -> Program {
    let fns = vrl::stdlib::all();
    compile(source, &fns)
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

const DD_REMAP: &str = r#"
.hostname = "vector"

if .status == "warning" {
  .thing = upcase(.hostname)
} else if .status == "notice" {
  .thung = downcase(.hostname)
} else {
  .nong = upcase(.hostname)
}

.matches = { "name": .message, "num": "2" }
.origin, .err = .hostname + "/" + .matches.name + "/" + .matches.num
"#;

#[allow(clippy::too_many_lines)]
fn bench_layout(c: &mut Criterion) {
    let mut g = c.benchmark_group("layout");
    let tz = TimeZone::default();
    let mut runtime = Runtime::default();
    let dd = build_dd();

    g.bench_function("build_dd8", |b| b.iter(|| black_box(build_dd())));

    g.bench_function("clone_dd8", |b| b.iter(|| black_box(dd.clone())));

    g.bench_function("clone_mutate_dd8", |b| {
        b.iter(|| {
            let mut v = dd.clone();
            if let Value::Object(map) = &mut v {
                map.insert(KeyString::from("hostname"), Value::from("vector"));
            }
            black_box(v)
        });
    });

    g.bench_function("drop_dd8", |b| {
        b.iter_batched(build_dd, drop, BatchSize::SmallInput);
    });

    g.bench_function("get6_dd8", |b| {
        let Value::Object(map) = &dd else {
            unreachable!()
        };
        b.iter(|| {
            for k in [
                "hostname", "status", "message", "thing", "service", "ddtags",
            ] {
                black_box(map.get(black_box(k)));
            }
        });
    });

    g.bench_function("iter_dd8", |b| {
        let Value::Object(map) = &dd else {
            unreachable!()
        };
        b.iter(|| {
            let mut n = 0usize;
            for (k, v) in map {
                n += k.len() + usize::from(v.is_bytes());
            }
            black_box(n)
        });
    });

    g.bench_function("json_encode_dd8", |b| {
        b.iter(|| black_box(serde_json::to_vec(&dd).unwrap()));
    });

    g.bench_function("json_decode_via_serde_value", |b| {
        b.iter(|| {
            let j: serde_json::Value = serde_json::from_str(black_box(JSON_EVENT)).unwrap();
            black_box(Value::from(j))
        });
    });

    g.bench_function("json_decode_direct", |b| {
        b.iter(|| black_box(serde_json::from_str::<Value>(black_box(JSON_EVENT)).unwrap()));
    });

    let remap = compile_vrl(DD_REMAP);
    g.bench_function("remap_dd_owned", |b| {
        b.iter_batched(
            || target(build_dd()),
            |mut t| {
                black_box(runtime.resolve(&mut t, &remap, &tz).unwrap());
                t
            },
            BatchSize::SmallInput,
        );
    });

    // Mirrors Vector cloning the event (remap with fallible/abortable program)
    // and the first write triggering a copy: clone is inside the timed region.
    g.bench_function("remap_dd_cloned", |b| {
        b.iter(|| {
            let mut t = target(dd.clone());
            black_box(runtime.resolve(&mut t, &remap, &tz).unwrap());
            t
        });
    });

    // Includes building + dropping the event: a whole-pipeline-ish proxy.
    g.bench_function("pipeline_dd", |b| {
        b.iter(|| {
            let mut t = target(build_dd());
            runtime.resolve(&mut t, &remap, &tz).unwrap();
            black_box(serde_json::to_vec(&t.value).unwrap())
        });
    });

    let syslog = compile_vrl(". = parse_syslog!(.message)");
    g.bench_function("parse_syslog", |b| {
        b.iter_batched(
            || {
                let mut map = ObjectMap::new();
                map.insert("message".into(), Value::from(SYSLOG_LINE));
                target(Value::Object(map))
            },
            |mut t| {
                black_box(runtime.resolve(&mut t, &syslog, &tz).unwrap());
                t
            },
            BatchSize::SmallInput,
        );
    });

    for n in [32usize, 128] {
        let keys = big_keys(n);
        let big = build_big(&keys);
        g.bench_function(format!("build_big{n}"), |b| {
            b.iter(|| black_box(build_big(&keys)));
        });
        g.bench_function(format!("get_all_big{n}"), |b| {
            let Value::Object(map) = &big else {
                unreachable!()
            };
            b.iter(|| {
                for k in &keys {
                    black_box(map.get(k.as_str()));
                }
            });
        });
    }

    g.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2))
        .sample_size(30)
        .noise_threshold(0.02);
    targets = bench_layout
);
criterion_main!(benches);
