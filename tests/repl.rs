use std::io::Write;
use std::process::{Command, Stdio};

const BANNER_MARKER: &str = "VECTOR    REMAP    LANGUAGE";

fn run_vrl_repl(input: Option<&str>, args: &[&str]) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_vrl"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn vrl process");

    let mut stdin = child
        .stdin
        .take()
        .expect("failed to take stdin for child vrl cli");

    if let Some(input) = input {
        stdin
            .write_all(format!("{input}\n").as_bytes())
            .expect("failed to write input to stdin");
    }

    // Send "exit" to close the REPL
    stdin
        .write_all(b"exit\n")
        .expect("failed to write to stdin");

    let output = child.wait_with_output().expect("failed to wait on child");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
// abs is just a random stdlib function
fn test_abs_works() {
    let stdout = run_vrl_repl(Some("abs(-1)"), &["-q"]);
    assert_eq!(stdout, "1\n\n");
}

#[test]
fn without_quiet_flag_prints_banner() {
    let stdout = run_vrl_repl(None, &[]);
    assert!(
        stdout.contains(BANNER_MARKER),
        "Expected banner to be printed without --quiet flag.\nStdout was:\n{stdout}"
    );
}

#[test]
fn with_quiet_long_flag_suppresses_banner() {
    let stdout = run_vrl_repl(None, &["--quiet"]);
    assert!(
        !stdout.contains(BANNER_MARKER),
        "Expected banner to be suppressed with --quiet flag.\nStdout was:\n{stdout}"
    );
}

#[test]
fn with_quiet_short_flag_suppresses_banner() {
    let stdout = run_vrl_repl(None, &["-q"]);
    assert!(
        !stdout.contains(BANNER_MARKER),
        "Expected banner to be suppressed with -q flag.\nStdout was:\n{stdout}"
    );
}
