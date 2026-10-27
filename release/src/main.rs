#![allow(clippy::print_stdout, clippy::print_stderr)]

use clap::{Parser, Subcommand};

mod changelog;
mod crates_io;
mod version;
mod workflow;

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
}

#[derive(Subcommand)]
enum Commands {
    /// Validate changelog fragment filenames
    CheckChangelog,
    /// Print the version in the repository's `Cargo.toml`
    Version,
    /// Print the version to tag if the `Cargo.toml` version went up since a commit.
    /// Prints nothing if the version did not change.
    BumpedVersion {
        /// Commit to compare with, usually the commit before the push.
        #[arg(long)]
        before: String,
    },
    /// Fail unless a tag is `v<version>` for the `Cargo.toml` version
    CheckTag {
        /// Tag name, for example `v1.2.3`.
        tag: String,
    },
    /// Print the release PR body for the `Cargo.toml` version
    PrBody {
        /// Release issue URL to link in the body. An empty value is ignored.
        #[arg(long)]
        issue: Option<String>,
    },
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

/// Prepare the release files: bump the version, refresh `Cargo.lock`, and move the
/// changelog fragments into `CHANGELOG.md`.
///
/// Git, the release PR, the tag, and the crates.io publish are handled by the
/// `release_prepare`, `release_autotag`, and `release_publish` workflows.
fn prepare(version_arg: Option<&str>, dry_run: bool) -> Result<(), String> {
    let root = repo_root();

    let manifest = root.join("Cargo.toml");
    let current = version::read_version(&manifest)?;
    let new_version = version::resolve(version_arg, &current)?;
    println!("Current version: {current}");
    println!("New version:     {new_version}");

    let changelog = changelog::Changelog::new(&root);

    if dry_run {
        println!("\n[dry-run] Generating changelog preview:\n");
        println!(
            "{}",
            changelog.generate_section(&new_version, changelog::PullRequestMetadata::Optional)?
        );
        return Ok(());
    }

    // Validate everything before the first file changes.
    let changelog_section =
        changelog.generate_section(&new_version, changelog::PullRequestMetadata::Required)?;
    crates_io::assert_not_published(&new_version)?;

    println!("Bumping version in Cargo.toml...");
    version::write_version(&manifest, &new_version)?;
    run("cargo", &["update", "-p", "vrl"], &root)?;

    println!("Generating changelog...");
    changelog.apply_section(&new_version, &changelog_section)?;

    println!("\nPrepared release {new_version}.");
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let root = repo_root();
    let result = match cli.command {
        Some(Commands::CheckChangelog) => changelog::Changelog::new(&root).check_fragments(),
        Some(Commands::Version) => {
            version::read_version(&root.join("Cargo.toml")).map(|version| println!("{version}"))
        }
        Some(Commands::BumpedVersion { before }) => workflow::bumped_version(&root, &before),
        Some(Commands::CheckTag { tag }) => workflow::check_tag(&root, &tag),
        Some(Commands::PrBody { issue }) => {
            workflow::pr_body(&root, issue.as_deref()).map(|body| println!("{body}"))
        }
        None => prepare(cli.version.as_deref(), cli.dry_run),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
