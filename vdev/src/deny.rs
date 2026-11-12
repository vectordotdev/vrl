//! Adapted from Vector's `vdev/src/commands/check/deny.rs`.
#![allow(clippy::print_stdout)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context as _, Result, bail, ensure};
use serde::Deserialize;

/// Check advisories, licenses, bans, and sources for crate dependencies.
#[derive(clap::Args)]
pub struct Cli {
    /// Only fail on errors absent from this checkout of the base revision.
    #[arg(long, value_name = "DIR")]
    base_dir: Option<PathBuf>,
}

impl Cli {
    pub fn exec(self) -> Result<()> {
        let Some(base_dir) = self.base_dir else {
            let status = Command::new("cargo").args(args("human")).status()?;
            ensure!(status.success(), "cargo deny failed: {status}");
            return Ok(());
        };
        let head_dir = std::env::current_dir()?.canonicalize()?;
        let base_dir = base_dir.canonicalize()?;
        ensure!(
            base_dir != head_dir && base_dir.join("Cargo.toml").is_file(),
            "--base-dir must be a separate checkout containing Cargo.toml"
        );

        let head = errors(&head_dir)?;
        if head.is_empty() {
            println!("No errors found.");
            return Ok(());
        }

        // Like Vector, scan head first. An advisory published between scans can
        // then appear only in base, so it cannot spuriously fail the PR.
        let base = errors(&base_dir)?;
        let (existing, new) = partition_errors(&head, &base);
        if !existing.is_empty() {
            println!("Errors that the base also has (ignored):");
            for error in existing {
                println!("{error}");
            }
        }
        if new.is_empty() {
            println!("No new errors compared to the base.");
            return Ok(());
        }
        println!("New errors compared to the base:");
        for error in new {
            println!("{error}");
        }
        bail!("This change adds cargo deny errors. Run `make check-deny` for details.")
    }
}

fn args(format: &str) -> [&str; 8] {
    // Preserve VRL's current lint policy. Vector additionally denies warnings
    // with an allowlist; that policy change is separate from base comparison.
    [
        "deny",
        "--log-level",
        "error",
        "--format",
        format,
        "--all-features",
        "check",
        "all",
    ]
}

fn errors(dir: &Path) -> Result<BTreeSet<DenyError>> {
    let output = Command::new("cargo")
        .args(args("json"))
        .current_dir(dir)
        .output()?;
    parse_output(&output).with_context(|| {
        format!(
            "cargo deny did not complete reliably in {}:\n{}{}",
            dir.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn parse_output(output: &Output) -> Result<BTreeSet<DenyError>> {
    let mut summary: Option<BTreeMap<String, Stats>> = None;
    let mut errors = BTreeSet::new();
    for line in String::from_utf8_lossy(&output.stderr)
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let line: Line = serde_json::from_str(line)?;
        match line.kind.as_str() {
            "summary" => summary = Some(serde_json::from_value(line.fields)?),
            "diagnostic" => {
                let diagnostic: Diagnostic = serde_json::from_value(line.fields)?;
                if diagnostic.severity == "error" {
                    errors.insert(DenyError::from(diagnostic));
                }
            }
            _ => {}
        }
    }
    // A nonzero exit alone cannot distinguish findings from a broken scan.
    let summary = summary.context("missing cargo deny summary")?;
    ensure!(!summary.is_empty(), "empty cargo deny summary");
    let has_errors = summary.values().any(|stats| stats.errors > 0);
    ensure!(
        has_errors != errors.is_empty()
            && output.status.code().is_some()
            && output.status.success() != has_errors,
        "exit status, summary and diagnostics disagree"
    );
    Ok(errors)
}

fn partition_errors<'a>(
    head: &'a BTreeSet<DenyError>,
    base: &BTreeSet<DenyError>,
) -> (Vec<&'a DenyError>, Vec<&'a DenyError>) {
    let base_keys: BTreeSet<_> = base.iter().map(|error| &error.key).collect();
    head.iter()
        .partition(|error| base_keys.contains(&error.key))
}

#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: String,
    fields: serde_json::Value,
}

#[derive(Deserialize)]
struct Stats {
    errors: u64,
}

#[derive(Deserialize)]
struct Diagnostic {
    severity: String,
    code: String,
    message: String,
    advisory: Option<Advisory>,
    #[serde(default)]
    graphs: Vec<Graph>,
    #[serde(default)]
    labels: Vec<Label>,
}

#[derive(Deserialize)]
struct Advisory {
    id: String,
}

#[derive(Deserialize)]
struct Graph {
    #[serde(rename = "Krate")]
    krate: Krate,
}

#[derive(Deserialize)]
struct Krate {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Label {
    message: String,
    span: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ErrorKey {
    // Like Vector, affected-version changes retain the same advisory identity.
    Advisory(String),
    Other {
        code: String,
        crates: Vec<String>,
        message: String,
        // Different rejected licenses share a generic message. Include labels,
        // excluding source locations that can move across revisions.
        labels: Vec<Label>,
    },
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DenyError {
    key: ErrorKey,
    description: String,
}

impl From<Diagnostic> for DenyError {
    fn from(diagnostic: Diagnostic) -> Self {
        let crates = diagnostic
            .graphs
            .iter()
            .map(|graph| format!("{} {}", graph.krate.name, graph.krate.version))
            .collect::<Vec<_>>()
            .join(", ");
        let (key, identifier) = if let Some(advisory) = diagnostic.advisory {
            (ErrorKey::Advisory(advisory.id.clone()), advisory.id)
        } else {
            let mut names: Vec<_> = diagnostic
                .graphs
                .into_iter()
                .map(|g| g.krate.name)
                .collect();
            names.sort();
            let mut labels = diagnostic.labels;
            labels.sort();
            (
                ErrorKey::Other {
                    code: diagnostic.code.clone(),
                    crates: names,
                    message: diagnostic.message.clone(),
                    labels,
                },
                diagnostic.code,
            )
        };
        Self {
            key,
            description: format!("{identifier}\t{crates}: {}", diagnostic.message),
        }
    }
}

impl fmt::Display for DenyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)
    }
}

#[cfg(test)]
mod tests;
