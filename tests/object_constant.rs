use vrl::btreemap;
use vrl::compiler::{TargetValue, TimeZone, compile, runtime::Runtime};
use vrl::value::{Secrets, Value};

fn run(source: &str, event: Value) -> Value {
    let program = compile(source, &vrl::stdlib::all())
        .expect("program compiles")
        .program;
    let mut target = TargetValue {
        value: event,
        metadata: Value::from(btreemap! {}),
        secrets: Secrets::new(),
    };
    let mut runtime = Runtime::default();
    runtime
        .resolve(&mut target, &program, &TimeZone::default())
        .expect("program resolves")
}

/// A fully-constant object literal must resolve to the same value whether or
/// not it was constant-cached at compile time.
#[test]
fn constant_object_literal_resolves() {
    let source = r#"
        map = { "emerg": 21, "alert": 19, "info": 9 }
        get(map, ["info"]) ?? null
    "#;
    let result = run(source, Value::from(btreemap! {}));
    assert_eq!(result, Value::from(9));
}

/// Object literals mixing constant and dynamic values must not use the
/// constant fast path, but still resolve correctly.
#[test]
fn mixed_object_literal_resolves() {
    let source = r#"
        map = { "static": 1, "dynamic": .field }
        map
    "#;
    let event = Value::from(btreemap! { "field" => 42 });
    let result = run(source, event);
    assert_eq!(
        result,
        Value::from(btreemap! { "static" => 1, "dynamic" => 42 })
    );
}

/// Repeated resolution of the same program must yield identical results
/// (guards the cached value being cloned, not moved).
#[test]
fn constant_object_literal_resolves_repeatedly() {
    let source = r#"
        map = { "a": 1, "b": 2 }
        map
    "#;
    let program = compile(source, &vrl::stdlib::all())
        .expect("program compiles")
        .program;
    let mut runtime = Runtime::default();
    for _ in 0..3 {
        let mut target = TargetValue {
            value: Value::from(btreemap! {}),
            metadata: Value::from(btreemap! {}),
            secrets: Secrets::new(),
        };
        let result = runtime
            .resolve(&mut target, &program, &TimeZone::default())
            .expect("program resolves");
        assert_eq!(result, Value::from(btreemap! { "a" => 1, "b" => 2 }));
    }
}
