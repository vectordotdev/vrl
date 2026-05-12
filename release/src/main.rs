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

    crates_io::assert_not_published(&new_version)?;

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
