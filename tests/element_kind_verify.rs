use vrl::compiler::compile;
use vrl::stdlib;

fn compiles(source: &str) -> bool {
    compile(source, &stdlib::all()).is_ok()
}

#[test]
fn encode_logfmt_rejects_non_bytes_element() {
    // P1 regression: `encode_logfmt` must reject non-string `fields_ordering` elements
    // at compile time, matching `encode_key_value`.
    assert!(
        !compiles(r#"encode_logfmt({"a": 1}, [1])"#),
        "should fail to compile"
    );
    assert!(
        !compiles(r#"encode_logfmt({"a": 1}, ["a", 2])"#),
        "should fail to compile"
    );
    // valid usage still compiles
    assert!(compiles(r#"encode_logfmt({"a": 1})"#), "should compile");
    assert!(
        compiles(r#"encode_logfmt({"a": 1}, ["a"])"#),
        "should compile"
    );
}

#[test]
fn encode_key_value_still_rejects_non_bytes_element() {
    assert!(
        !compiles(r#"encode_key_value({"a": 1}, [1])"#),
        "should fail to compile"
    );
    assert!(
        compiles(r#"encode_key_value({"a": 1}, ["a"])"#),
        "should compile"
    );
}
