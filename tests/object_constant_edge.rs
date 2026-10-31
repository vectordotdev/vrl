//! Edge-case tests for the constant object literal cache.
//!
//! Every test must behave identically to the uncached resolve path.
//! Run on both patched and unpatched vrl to compare semantics.

use vrl::btreemap;
use vrl::compiler::{TargetValue, TimeZone, compile, runtime::Runtime};
use vrl::value::{Secrets, Value};

fn run(source: &str, event: Value) -> Value {
    let program = compile(source, &vrl::stdlib::all())
        .unwrap_or_else(|e| panic!("program compiles: {e:?}"))
        .program;
    let mut target = TargetValue {
        value: event,
        metadata: Value::from(btreemap! {}),
        secrets: Secrets::new(),
    };
    let mut runtime = Runtime::default();
    runtime
        .resolve(&mut target, &program, &TimeZone::default())
        .unwrap_or_else(|e| panic!("program resolves: {e:?}"))
}

/// Variable reassigned inside a conditional branch: the object built from the
/// variable must follow the runtime value, not a compile-time snapshot.
#[test]
fn variable_changed_in_branch() {
    let source = r#"
        a = 5
        if bool(.cond) ?? false { a = 6 }
        { "x": a }
    "#;
    let res_true = run(source, Value::from(btreemap! { "cond" => true }));
    assert_eq!(res_true, Value::from(btreemap! { "x" => 6 }));

    let res_false = run(source, Value::from(btreemap! { "cond" => false }));
    assert_eq!(res_false, Value::from(btreemap! { "x" => 5 }));
}

/// Variable reassigned from a dynamic value after a constant assignment.
#[test]
fn variable_reassigned_dynamic() {
    let source = r#"
        a = 5
        a = .dyn
        { "x": a }
    "#;
    let res = run(source, Value::from(btreemap! { "dyn" => 42 }));
    assert_eq!(res, Value::from(btreemap! { "x" => 42 }));
}

/// Mutating a constant literal after assignment must not leak into any cache.
#[test]
fn literal_mutated_after_assignment() {
    let source = r#"
        map = { "x": 1 }
        map.x = 2
        map.y = 3
        map
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! { "x" => 2, "y" => 3 }));
}

/// The same source line resolved twice must not share state between runs.
#[test]
fn mutation_does_not_leak_across_resolves() {
    let source = r#"
        map = { "x": 1 }
        map.x = 2
        map
    "#;
    let program = compile(source, &vrl::stdlib::all())
        .expect("compiles")
        .program;
    let mut runtime = Runtime::default();
    for _ in 0..3 {
        let mut target = TargetValue {
            value: Value::from(btreemap! {}),
            metadata: Value::from(btreemap! {}),
            secrets: Secrets::new(),
        };
        let res = runtime
            .resolve(&mut target, &program, &TimeZone::default())
            .expect("resolves");
        assert_eq!(res, Value::from(btreemap! { "x" => 2 }));
    }
}

/// Merging into a constant literal must behave like a fresh map.
#[test]
fn literal_merged_after_assignment() {
    let source = r#"
        map = { "x": 1 }
        merged = merge(map, { "y": 2 })
        merged
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! { "x" => 1, "y" => 2 }));
}

/// A non-deterministic function (now) must never be baked into the cache.
#[test]
fn now_not_baked_into_constant() {
    let source = r#"
        { "ts": now() }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    let Value::Object(map) = &res else {
        panic!("expected object, got {res:?}");
    };
    assert!(map.get("ts").is_some(), "ts must be present");
}

/// Nested constant literals (object in object, arrays).
#[test]
fn nested_constant_literals() {
    let source = r#"
        { "a": { "b": [1, 2, 3] }, "c": "s" }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(
        res,
        Value::from(btreemap! {
            "a" => Value::from(btreemap! { "b" => Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]) }),
            "c" => Value::from("s"),
        })
    );
}

/// Constant arithmetic inside a literal folds to the same value as runtime.
#[test]
fn constant_op_inside_literal() {
    let source = r#"
        { "x": 1 + 2, "y": 2 * 3 }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! { "x" => 3, "y" => 6 }));
}

/// Fallible constant arithmetic (div by zero) must not be cached to a wrong
/// value; the runtime error path must be preserved.
#[test]
fn fallible_constant_op_not_cached() {
    let source = r#"
        { "x": 1 / 0 ?? "fallback" }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! { "x" => "fallback" }));
}

/// Empty object literal.
#[test]
fn empty_object_literal() {
    let res = run("{}", Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! {}));
}

/// Constant function call (`parse_float`) inside a literal keeps folding.
#[test]
fn constant_function_inside_literal() {
    let source = r#"
        { "x": parse_float("1.5") ?? -1.0 }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    let Value::Object(map) = res else {
        panic!("expected object");
    };
    let Some(Value::Float(f)) = map.get("x") else {
        panic!("expected float x");
    };
    assert!((f.into_inner() - 1.5_f64).abs() < f64::EPSILON);
}

/// Mutation via `del` on a constant literal clone.
#[test]
fn del_on_constant_literal() {
    let source = r#"
        map = { "x": 1, "y": 2 }
        del(map.y)
        map
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(btreemap! { "x" => 1 }));
}

/// Literal used as a function argument then mutated by that function
/// (set! semantics): the cached literal must not be affected across runs.
#[test]
fn literal_passed_to_mutation_function() {
    let source = r#"
        map = { "x": 1 }
        map = set!(map, ["y"], 2)
        map
    "#;
    let program = compile(source, &vrl::stdlib::all())
        .expect("compiles")
        .program;
    let mut runtime = Runtime::default();
    for _ in 0..3 {
        let mut target = TargetValue {
            value: Value::from(btreemap! {}),
            metadata: Value::from(btreemap! {}),
            secrets: Secrets::new(),
        };
        let res = runtime
            .resolve(&mut target, &program, &TimeZone::default())
            .expect("resolves");
        assert_eq!(res, Value::from(btreemap! { "x" => 1, "y" => 2 }));
    }
}

/// Variable read at a path inside a literal, where the variable is constant.
#[test]
fn variable_path_inside_literal() {
    let source = r#"
        m = { "x": 1 }
        { "y": m.x, "z": .dyn }
    "#;
    let res = run(source, Value::from(btreemap! { "dyn" => 7 }));
    assert_eq!(res, Value::from(btreemap! { "y" => 1, "z" => 7 }));
}

/// Constant literal inside a closure body resolves correctly per call.
#[test]
fn constant_literal_inside_closure() {
    let source = r#"
        lookup = { "a": 1, "b": 2 }
        map_values([1, 2]) -> |value| {
            get(lookup, ["a"]) ?? 0
        }
    "#;
    let res = run(source, Value::from(btreemap! {}));
    assert_eq!(res, Value::from(vec![Value::from(1), Value::from(1)]));
}

/// Regression guard for `vectordotdev/vrl#1799` semantics: an object entry that
/// runs side effects mutating a variable must be observed by later entries.
/// The constant cache must not freeze variable-based entries at compile time.
#[test]
fn side_effect_in_object_observed_by_later_entries() {
    let source = r#"
        pat = "foo"
        obj = { "b": pat, "a": { pat = "bar"; "side" } }
        obj
    "#;
    let res = run(source, Value::from(btreemap! {}));
    // entries resolve in BTreeMap key order: "a" runs first (sets pat="bar"),
    // then "b" must observe the runtime value "bar", not the frozen "foo".
    assert_eq!(
        res,
        Value::from(btreemap! { "a" => Value::from("side"), "b" => Value::from("bar") })
    );
}
