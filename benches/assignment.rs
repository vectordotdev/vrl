use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{
    BatchSize, BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use vrl::compiler::state::{ExternalEnv, RuntimeState};
use vrl::compiler::{
    CompileConfig, Context, Program, TargetValue, TimeZone, compile, compile_with_external,
};
use vrl::stdlib::all;
use vrl::value::kind::{Collection, Field, Kind};
use vrl::value::{KeyString, ObjectMap, Secrets, Value};

fn make_target(value: Value) -> TargetValue {
    TargetValue {
        value,
        metadata: Value::Object(ObjectMap::new()),
        secrets: Secrets::default(),
    }
}

/// Builds a ~1 MB structured JSON object payload with `"nested": { "deep": { "leaf": "initial" } }`
/// and 1,000 structured sibling fields (~1 KB each).
fn build_set_remove_payload() -> Value {
    let mut root = ObjectMap::new();

    let mut leaf = ObjectMap::new();
    leaf.insert(KeyString::from("leaf"), Value::from("initial"));

    let mut deep = ObjectMap::new();
    deep.insert(KeyString::from("deep"), Value::Object(leaf));

    root.insert(KeyString::from("nested"), Value::Object(deep));

    let padding = "a".repeat(950);
    for i in 0..1000_i64 {
        let mut entry = ObjectMap::new();
        entry.insert(KeyString::from("id"), Value::Integer(i));
        entry.insert(KeyString::from("active"), Value::Boolean(i % 2 == 0));
        entry.insert(
            KeyString::from("tags"),
            Value::Array(vec![
                Value::from("alpha"),
                Value::from("beta"),
                Value::from("gamma"),
            ]),
        );
        entry.insert(
            KeyString::from("message"),
            Value::from(format!("payload-{i:04}-{padding}")),
        );
        root.insert(
            KeyString::from(format!("field_{i:04}")),
            Value::Object(entry),
        );
    }

    Value::Object(root)
}

/// Builds a ~1 MB object payload with `"events"` containing 100 structured items (~10 KB each).
fn build_unnest_payload() -> Value {
    let padding = "b".repeat(10_000);
    let events = (0..100_i64)
        .map(|i| {
            let mut attributes = ObjectMap::new();
            attributes.insert(KeyString::from("region"), Value::from("us-east-1"));
            attributes.insert(
                KeyString::from("detail"),
                Value::from(format!("event-{i:03}-{padding}")),
            );

            let mut item = ObjectMap::new();
            item.insert(KeyString::from("id"), Value::Integer(i));
            item.insert(
                KeyString::from("timestamp"),
                Value::from("2026-11-23T00:00:00Z"),
            );
            item.insert(KeyString::from("level"), Value::from("info"));
            item.insert(
                KeyString::from("tags"),
                Value::Array(vec![Value::from("prod"), Value::from("edge")]),
            );
            item.insert(KeyString::from("attributes"), Value::Object(attributes));
            Value::Object(item)
        })
        .collect::<Vec<_>>();

    let mut root = ObjectMap::new();
    root.insert(KeyString::from("host"), Value::from("bench-host"));
    root.insert(KeyString::from("source"), Value::from("vrl-bench"));
    root.insert(KeyString::from("events"), Value::Array(events));
    Value::Object(root)
}

fn unnest_external_env() -> ExternalEnv {
    let mut fields = BTreeMap::new();
    fields.insert(Field::from("events"), Kind::array(Collection::any()));
    ExternalEnv::new_with_kind(
        Kind::object(Collection::from_parts(fields, Kind::any())),
        Kind::any_object(),
    )
}

fn compile_program(source: &str, external: Option<&ExternalEnv>) -> Program {
    let fns = all();
    let compiled = match external {
        Some(env) => compile_with_external(source, &fns, env, CompileConfig::default()),
        None => compile(source, &fns),
    };
    compiled
        .unwrap_or_else(|err| panic!("failed to compile `{source}`: {err:?}"))
        .program
}

fn bench_program(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    source: &str,
    external: Option<&ExternalEnv>,
    initial_value: &Value,
) {
    let program = compile_program(source, external);
    let tz = TimeZone::default();

    group.bench_function(name, |b| {
        b.iter_batched(
            || (make_target(initial_value.clone()), RuntimeState::default()),
            |(mut target, mut state)| {
                let mut ctx = Context::new(&mut target, &mut state, &tz);
                let result = program
                    .resolve(&mut ctx)
                    .unwrap_or_else(|err| panic!("benchmark `{name}` failed at runtime: {err:?}"));
                (target, black_box(result))
            },
            BatchSize::LargeInput,
        );
    });
}

fn bench_assignment_1mb(c: &mut Criterion) {
    let set_remove_payload = build_set_remove_payload();
    let unnest_payload = build_unnest_payload();
    let unnest_env = unnest_external_env();

    let cases: &[(&str, &str, Option<&ExternalEnv>, &Value)] = &[
        (
            "set_non_final",
            r#". = set!(., ["nested", "deep", "leaf"], "x"); null"#,
            None,
            &set_remove_payload,
        ),
        (
            "set_final",
            r#". = set!(., ["nested", "deep", "leaf"], "x")"#,
            None,
            &set_remove_payload,
        ),
        (
            "remove_non_final",
            r#". = remove!(., ["nested", "deep", "leaf"]); null"#,
            None,
            &set_remove_payload,
        ),
        (
            "remove_final",
            r#". = remove!(., ["nested", "deep", "leaf"])"#,
            None,
            &set_remove_payload,
        ),
        (
            "unnest_non_final",
            ". = unnest(.events); null",
            Some(&unnest_env),
            &unnest_payload,
        ),
        (
            "unnest_final",
            ". = unnest(.events)",
            Some(&unnest_env),
            &unnest_payload,
        ),
    ];

    let mut group = c.benchmark_group("assignment_1mb");
    group.sample_size(20);

    for &(name, source, external, payload) in cases {
        bench_program(&mut group, name, source, external, payload);
    }

    group.finish();
}

criterion_group!(benches, bench_assignment_1mb);
criterion_main!(benches);
