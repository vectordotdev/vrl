#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand};

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
}

#[derive(Subcommand)]
enum Commands {
    /// Validate changelog fragment filenames
    CheckChangelog,
}

/// Run a command, printing it first. Returns stdout as a String.
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

    // 1. Resolve version
    let current = version::read_version(&root)?;
    let new_version = version::resolve(version_arg, &current)?;
    println!("Current version: {current}");
    println!("New version:     {new_version}");

    // 2. Validate not already published
    crates_io::assert_not_published(&new_version)?;

    let changelog = changelog::Changelog::new(&root);

    if dry_run {
        println!(
            "\n[dry-run] Would create branch, bump version, generate changelog, publish, tag, and create PR."
        );
        println!("[dry-run] Generating changelog preview:\n");
        let preview = changelog.generate_section(&new_version)?;
        println!("{preview}");
        return Ok(());
    }

    // 3. Create branch
    let branch = format!("prepare-{new_version}-release");
    println!("\nCreating branch: {branch}");
    run("git", &["checkout", "-b", &branch], &root)?;

    // 4. Bump version
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

    // 5. Generate changelog
    println!("Generating changelog...");
    changelog.generate_and_apply(&new_version)?;
    run(
        "git",
        &["commit", "-a", "-m", "chore(releasing): generate changelog"],
        &root,
    )?;

    // 6. Pause for review
    pause_for_review();

    // 7. Publish to crates.io
    println!("Publishing to crates.io...");
    run("cargo", &["publish"], &root)?;
    println!("Published vrl v{new_version} to crates.io.");

    // 8. Tag
    let tag = format!("v{new_version}");
    run(
        "git",
        &["tag", "-a", &tag, "-m", &format!("Release {new_version}")],
        &root,
    )?;

    // 9. Push branch + tag
    println!("Pushing...");
    run("git", &["push", "-u", "origin", &branch], &root)?;
    run("git", &["push", "origin", &tag], &root)?;

    // 10. Create PR
    println!("Creating pull request...");
    let title = format!("chore(releasing): Prepare {new_version} release");
    let mut body = format!(
        "Release {new_version}\n\nPublished to crates.io: https://crates.io/crates/vrl/{new_version}\nTag: `{tag}`"
    );
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
        None => {
            // Default: run the release flow
            // Parse release-specific args manually since they're positional on the default command
            let args: Vec<String> = std::env::args().collect();
            let mut version_arg: Option<String> = None;
            let mut dry_run = false;
            let mut issue: Option<String> = None;

            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--dry-run" => dry_run = true,
                    "--issue" | "-i" => {
                        i += 1;
                        issue = Some(
                            args.get(i)
                                .cloned()
                                .ok_or_else(|| "expected URL after --issue".to_string())
                                .unwrap(),
                        );
                    }
                    arg if !arg.starts_with('-') => {
                        version_arg = Some(arg.to_string());
                    }
                    _ => {}
                }
                i += 1;
            }

            release(version_arg.as_deref(), dry_run, issue.as_deref())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
