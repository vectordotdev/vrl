//! Compile-time tests using trybuild.
//!
//! These tests verify that macros and other compile-time constructs correctly
//! reject invalid usage with appropriate error messages.
//!
//! # Organization
//!
//! Test files are organized to mirror the source code structure:
//! - `src/compiler/function.rs` → `tests/compiler/function/*/`
//! - `src/parser/foo.rs` → `tests/parser/foo/*/`
//!
//! # Adding New Tests
//!
//! 1. Create test files in the appropriate directory matching the source structure
//! 2. Add a new test function following the naming pattern: `{module}_{submodule}_{feature}`
//! 3. Use `t.pass()` for tests that should compile successfully
//! 4. Use `t.compile_fail()` for tests that should fail with specific error messages

/// Tests for the example! macro defined in src/compiler/function.rs
///
/// The example! macro accepts fields in any order and validates:
/// - All required fields are present (title, source, result)
/// - No duplicate fields
/// - No unknown fields
#[test]
fn compiler_function_example() {
    let t = trybuild::TestCases::new();

    // Valid usage
    t.pass("tests/compiler/function/example/pass.rs");

    // Duplicate field errors
    t.compile_fail("tests/compiler/function/example/duplicate_title.rs");
    t.compile_fail("tests/compiler/function/example/duplicate_source.rs");
    t.compile_fail("tests/compiler/function/example/duplicate_result.rs");

    // Missing field errors
    t.compile_fail("tests/compiler/function/example/missing_title.rs");
    t.compile_fail("tests/compiler/function/example/missing_source.rs");
    t.compile_fail("tests/compiler/function/example/missing_result.rs");

    // Unknown field errors
    t.compile_fail("tests/compiler/function/example/unknown_field.rs");
}

// Add more test functions here as needed:
//
// #[test]
// fn parser_something() {
//     let t = trybuild::TestCases::new();
//     t.pass("tests/parser/something/pass.rs");
//     t.compile_fail("tests/parser/something/fail.rs");
// }
