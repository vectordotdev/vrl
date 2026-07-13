#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand};
use indoc::formatdoc;

mod changelog;
mod crates_io;
mod version;

use std::io::{Write, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "release", about = "VRL release tooling")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Version to release: "major", "minor", "patch", or an exact version like "1.2.3".
    /// Defaults to minor bump.
    version: Option<String>,

    /// Preview without making any changes.
    #[arg(long)]
    dry_run: bool,

    /// GitHub issue link to include in the PR body.
    #[arg(long, short)]
    issue: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate changelog fragment filenames
    CheckChangelog,
}

fn run(cmd: &str, args: &[&str], cwd: &Path) -> Result<String, String> {
    let display = format!("{cmd} {}", args.join(" "));
    println!("  $ {display}");

    let output = Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to run `{display}`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "`{display}` failed (exit {}):\n{stdout}{stderr}",
            output.status
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Check everything that can fail *before* we mutate git or files.
///
/// Any half-completed release run — commits made but publish/tag skipped — has to be
/// unwound by hand, so it's cheap to be paranoid up front. Order the checks so the
/// fastest / most likely to fail run first.
fn preflight(root: &Path, new_version: &semver::Version) -> Result<(), String> {
    println!("Running pre-flight checks...");

    // Git: clean tree, on main, up-to-date with origin/main.
    let status = run("git", &["status", "--porcelain"], root)?;
    if !status.trim().is_empty() {
        return Err(format!(
            "Working tree is not clean. Commit or stash first:\n{status}"
        ));
    }
    let branch = run("git", &["rev-parse", "--abbrev-ref", "HEAD"], root)?
        .trim()
        .to_string();
    if branch != "main" {
        return Err(format!(
            "Must run release from `main` (currently on `{branch}`)."
        ));
    }
    run("git", &["fetch", "origin", "main"], root)?;
    let local = run("git", &["rev-parse", "main"], root)?.trim().to_string();
    let remote = run("git", &["rev-parse", "origin/main"], root)?
        .trim()
        .to_string();
    if local != remote {
        return Err(format!(
            "Local `main` ({local:.10}) is not in sync with `origin/main` ({remote:.10}). Pull first."
        ));
    }

    // `gh` CLI authenticated (needed to open the PR).
    run("gh", &["auth", "status"], root)
        .map_err(|e| format!("`gh` is not authenticated. Run `gh auth login` first.\n{e}"))?;

    // Every fragment currently in changelog.d/ must parse — that's what the
    // release will consume. `check_fragments` is the PR-time validator that
    // diffs against origin/main, so it would see zero additions on a synced
    // release branch and abort here; don't use it for release-time validation.
    changelog::Changelog::new(root).validate_fragments_on_disk()?;

    // Version not already published.
    crates_io::assert_not_published(new_version)?;

    // Interactive `cargo login` runs last so the releaser only has to fetch a
    // token once we know the automated checks pass. No crates.io endpoint
    // accepts an API token in a non-mutating way, so we can't verify a stored
    // token offline; refreshing it here guarantees the token about to be used
    // is fresh and valid.
    println!(
        "\nGrab a token from https://crates.io/me — ensure the pasted token has publish permissions."
    );
    let status = Command::new("cargo")
        .arg("login")
        .current_dir(root)
        .status()
        .map_err(|e| format!("Failed to run `cargo login`: {e}"))?;
    if !status.success() {
        return Err(format!("`cargo login` failed (exit {status})."));
    }

    println!("\nPre-flight checks passed.\n");
    Ok(())
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("release crate must be inside the repo")
        .to_path_buf()
}

fn pause_for_review() {
    println!();
    println!("Review the changelog now. Edit CHANGELOG.md if needed.");
    println!("Press Enter to continue, or Ctrl-C to abort.");
    print!("> ");
    stdout().flush().unwrap();
    let mut buf = String::new();
    stdin().read_line(&mut buf).unwrap();
}

fn release(version_arg: Option<&str>, dry_run: bool, issue: Option<&str>) -> Result<(), String> {
    let root = repo_root();

    let current = version::read_version(&root)?;
    let new_version = version::resolve(version_arg, &current)?;
    println!("Current version: {current}");
    println!("New version:     {new_version}");

    let changelog = changelog::Changelog::new(&root);

    if dry_run {
        println!("\n[dry-run] Generating changelog preview:\n");
        println!("{}", changelog.generate_section(&new_version)?);
        return Ok(());
    }

    preflight(&root, &new_version)?;

    let branch = format!("prepare-{new_version}-release");
    println!("\nCreating branch: {branch}");
    run("git", &["checkout", "-b", &branch], &root)?;

    println!("Bumping version in Cargo.toml...");
    version::write_version(&root, &new_version)?;
    run("cargo", &["update", "-p", "vrl"], &root)?;
    run(
        "git",
        &[
            "commit",
            "-a",
            "-m",
            &format!("chore(releasing): bump version to {new_version}"),
        ],
        &root,
    )?;

    println!("Generating changelog...");
    changelog.generate_and_apply(&new_version)?;

    pause_for_review();

    run(
        "git",
        &["commit", "-a", "-m", "chore(releasing): generate changelog"],
        &root,
    )?;

    println!("Publishing to crates.io...");
    run("cargo", &["publish"], &root)?;
    println!("Published vrl v{new_version} to crates.io.");

    let tag = format!("v{new_version}");
    run(
        "git",
        &["tag", "-a", &tag, "-m", &format!("Release {new_version}")],
        &root,
    )?;

    println!("Pushing...");
    run("git", &["push", "-u", "origin", &branch], &root)?;
    run("git", &["push", "origin", &tag], &root)?;

    println!("Creating pull request...");
    let title = format!("chore(releasing): Prepare {new_version} release");
    let mut body = formatdoc! {"
        Release {new_version}

        Published to crates.io: https://crates.io/crates/vrl/{new_version}
        Tag: `{tag}`"
    };
    if let Some(link) = issue {
        body.push_str(&format!("\n\nRelated issue: {link}"));
    }
    run(
        "gh",
        &[
            "pr",
            "create",
            "--title",
            &title,
            "--body",
            &body,
            "--head",
            &branch,
            "--base",
            "main",
            "--label",
            "no-changelog",
        ],
        &root,
    )?;

    println!("\nRelease {new_version} complete!");
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Some(Commands::CheckChangelog) => changelog::Changelog::new(&repo_root()).check_fragments(),
        None => release(cli.version.as_deref(), cli.dry_run, cli.issue.as_deref()),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
