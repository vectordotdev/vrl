use std::path::Path;
use std::process::Command;

/// Run `git` in `repo_root` and return its stdout.
///
/// Unlike `main::run`, this does not echo the command to stdout, so callers can
/// print values that workflows consume.
pub fn git(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let display = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run `{display}`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{display}` failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("`{display}` printed invalid UTF-8: {e}"))
}
