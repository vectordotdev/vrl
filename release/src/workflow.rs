//! Steps of the `release_prepare`, `release_autotag`, and `release_publish` workflows.
//!
//! Each function backs one subcommand. Values the workflow consumes go to stdout;
//! progress messages go to stderr.

use crate::version;
use indoc::formatdoc;
use std::fmt::Write as _;
use std::path::Path;

/// Print the version to tag if the `Cargo.toml` version went up since `before`.
///
/// Prints nothing if the version did not change. Fails if it went down.
pub fn bumped_version(root: &Path, before: &str) -> Result<(), String> {
    let previous = version::read_version_at(root, before)?;
    let current = version::read_version(&root.join("Cargo.toml"))?;

    match version::detect_bump(&previous, &current)? {
        Some(new) => {
            eprintln!("Version changed from {previous} to {new}.");
            println!("{new}");
        }
        None => eprintln!("Version unchanged ({current}), nothing to tag."),
    }
    Ok(())
}

/// Fail unless `tag` is `v<version>` for the `Cargo.toml` version.
pub fn check_tag(root: &Path, tag: &str) -> Result<(), String> {
    let version = version::read_version(&root.join("Cargo.toml"))?;
    if tag != format!("v{version}") {
        return Err(format!(
            "Tag {tag} does not match Cargo.toml version {version}."
        ));
    }
    eprintln!("Tag {tag} matches Cargo.toml version {version}.");
    Ok(())
}

/// Body of the release PR for the `Cargo.toml` version.
pub fn pr_body(root: &Path, issue: Option<&str>) -> Result<String, String> {
    let version = version::read_version(&root.join("Cargo.toml"))?;
    let mut body = formatdoc! {"
        Automated release preparation for VRL {version}.

        - Bumps the version to `{version}`.
        - Moves the `changelog.d/` fragments into `CHANGELOG.md`. Push edits to this branch to change the changelog.

        Merging this PR tags the merge commit as `v{version}` and publishes the crate to crates.io."
    };
    if let Some(issue) = issue.filter(|issue| !issue.is_empty()) {
        write!(body, "\n\nRelated issue: {issue}").expect("writing to a String cannot fail");
    }
    Ok(body)
}
