use super::*;
use serde_json::{Value, json};

fn diagnostic(id: Option<&str>, version: &str) -> Value {
    json!({
        "type": "diagnostic",
        "fields": {
            "severity": "error",
            "code": "rejected",
            "message": "failed to satisfy license requirements",
            "advisory": id.map(|id| json!({"id": id})),
            "graphs": [{"Krate": {"name": "example", "version": version}}],
        }
    })
}

fn output(diagnostics: &[Value], code: i32) -> Output {
    let error_count = diagnostics
        .iter()
        .filter(|line| line["fields"]["severity"] == "error")
        .count();
    let mut lines = diagnostics.to_vec();
    lines.push(json!({"type": "summary", "fields": {
        "advisories": {"errors": error_count, "warnings": 0}
    }}));
    #[cfg(unix)]
    let status = {
        use std::os::unix::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code << 8)
    };
    #[cfg(windows)]
    let status = {
        use std::os::windows::process::ExitStatusExt as _;
        std::process::ExitStatus::from_raw(code.cast_unsigned())
    };
    Output {
        status,
        stdout: Vec::new(),
        stderr: lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes(),
    }
}

fn findings(diagnostics: &[Value]) -> BTreeSet<DenyError> {
    let has_errors = diagnostics
        .iter()
        .any(|line| line["fields"]["severity"] == "error");
    parse_output(&output(diagnostics, i32::from(has_errors))).unwrap()
}

#[test]
fn clean_scan_passes() {
    assert!(findings(&[]).is_empty());
}

#[test]
fn advisory_version_change_is_not_new() {
    let head = findings(&[diagnostic(Some("RUSTSEC-2026-0001"), "1.0.1")]);
    let base = findings(&[diagnostic(Some("RUSTSEC-2026-0001"), "1.0.0")]);
    let (existing, new) = partition_errors(&head, &base);
    assert_eq!(existing.len(), 1);
    assert!(new.is_empty());
}

#[test]
fn new_advisory_is_reported_alongside_existing_one() {
    let old = diagnostic(Some("RUSTSEC-2026-0001"), "1.0.0");
    let head = findings(&[old.clone(), diagnostic(Some("RUSTSEC-2026-0002"), "1.0.0")]);
    let base = findings(&[old]);
    let (existing, new) = partition_errors(&head, &base);
    assert_eq!(existing.len(), 1);
    assert_eq!(new.len(), 1);
    assert!(new[0].description.contains("RUSTSEC-2026-0002"));
}

#[test]
fn removing_a_finding_passes() {
    let head = findings(&[]);
    let base = findings(&[diagnostic(Some("RUSTSEC-2026-0001"), "1.0.0")]);
    assert!(partition_errors(&head, &base).1.is_empty());
}

#[test]
fn existing_license_error_passes() {
    let errors = findings(&[diagnostic(None, "1.0.0")]);
    assert!(partition_errors(&errors, &errors).1.is_empty());
}

#[test]
fn newly_rejected_license_is_not_hidden_by_an_existing_rejection() {
    let mut head_error = diagnostic(None, "1.0.0");
    head_error["fields"]["labels"] = json!([{"message": "rejected", "span": "MIT"}]);
    let mut base_error = diagnostic(None, "1.0.0");
    base_error["fields"]["labels"] = json!([{"message": "rejected", "span": "BSD-3-Clause"}]);
    let head = findings(&[head_error]);
    let base = findings(&[base_error]);
    assert_eq!(partition_errors(&head, &base).1.len(), 1);
}

#[test]
fn moved_diagnostic_does_not_become_new() {
    let mut head_error = diagnostic(None, "1.0.0");
    head_error["fields"]["labels"] = json!([{"message": "rejected", "span": "MIT", "line": 9}]);
    let mut base_error = head_error.clone();
    base_error["fields"]["labels"][0]["line"] = json!(5);
    let head = findings(&[head_error]);
    let base = findings(&[base_error]);
    assert!(partition_errors(&head, &base).1.is_empty());
}

#[test]
fn warnings_remain_nonfatal() {
    let mut warning = diagnostic(None, "1.0.0");
    warning["fields"]["severity"] = json!("warning");
    assert!(findings(&[warning]).is_empty());
}

#[test]
fn different_check_types_have_different_error_exit_codes() {
    assert_eq!(
        parse_output(&output(&[diagnostic(None, "1.0.0")], 4))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn summary_is_required_even_with_valid_diagnostics() {
    let mut output = output(&[], 1);
    output.stderr = diagnostic(None, "1.0.0").to_string().into_bytes();
    assert!(parse_output(&output).is_err());
}

#[test]
fn invalid_json_fails() {
    let mut output = output(&[], 1);
    output.stderr = b"not json".to_vec();
    assert!(parse_output(&output).is_err());
}

#[test]
fn malformed_diagnostic_fails() {
    let mut error = diagnostic(None, "1.0.0");
    error["fields"].as_object_mut().unwrap().remove("message");
    assert!(parse_output(&output(&[error], 1)).is_err());
}

#[test]
fn status_must_agree_with_findings() {
    assert!(parse_output(&output(&[], 1)).is_err());
    assert!(parse_output(&output(&[diagnostic(None, "1.0.0")], 0)).is_err());
}

#[test]
fn summary_errors_without_diagnostics_fail() {
    let mut output = output(&[diagnostic(None, "1.0.0")], 1);
    output.stderr = String::from_utf8(output.stderr)
        .unwrap()
        .lines()
        .last()
        .unwrap()
        .as_bytes()
        .to_vec();
    assert!(parse_output(&output).is_err());
}

#[test]
fn empty_summary_fails() {
    let mut output = output(&[], 0);
    output.stderr = br#"{"type":"summary","fields":{}}"#.to_vec();
    assert!(parse_output(&output).is_err());
}

#[test]
fn self_comparison_is_rejected() {
    let error = Cli {
        base_dir: Some(PathBuf::from(".")),
    }
    .exec()
    .unwrap_err();
    assert!(error.to_string().contains("separate checkout"));
}
