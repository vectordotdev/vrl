//! Steps of the `release_prepare`, `release_autotag`, and `release_publish` workflows.
//!
//! Each function backs one subcommand. Values the workflow consumes go to stdout;
//! progress messages go to stderr.

use crate::git::git;
use crate::version;
use indoc::formatdoc;
use std::fmt::Write as _;
use std::path::Path;

/// The commit in a push or merge group that raises the `Cargo.toml` version.
#[derive(Debug, PartialEq)]
struct ReleaseCommit {
    sha: String,
    version: semver::Version,
}

/// Find the commit in `before..HEAD` that raises the `Cargo.toml` version.
///
/// The merge queue can land several PRs in one push, so the release commit is not
/// always `HEAD`. Fails if the version goes down, if more than one commit raises it,
/// or if changelog fragments remain at the release commit. Remaining fragments mean
/// that a PR merged after the release PR was prepared, and the changelog misses it.
fn find_release_commit(root: &Path, before: &str) -> Result<Option<ReleaseCommit>, String> {
    let range = format!("{before}..HEAD");
    let commits = git(
        root,
        &[
            "rev-list",
            "--reverse",
            "--first-parent",
            &range,
            "--",
            "Cargo.toml",
        ],
    )?;

    let mut found: Option<ReleaseCommit> = None;
    for sha in commits.lines() {
        let previous = version::read_version_at(root, &format!("{sha}^"))?;
        let current = version::read_version_at(root, sha)?;
        let Some(version) = version::detect_bump(&previous, &current)? else {
            continue;
        };
        if let Some(first) = &found {
            return Err(format!(
                "More than one commit in {range} raises the version: {} ({}) and {sha} ({version}).",
                first.sha, first.version
            ));
        }
        found = Some(ReleaseCommit {
            sha: sha.to_owned(),
            version,
        });
    }

    let Some(release) = found else {
        return Ok(None);
    };
    let fragments = fragments_at(root, &release.sha)?;
    if !fragments.is_empty() {
        return Err(format!(
            "Release commit {} for {} leaves changelog fragments that are not in CHANGELOG.md:\n  {}\n\
             A PR merged after the release PR was prepared. Close the release PR, delete its \
             branch, and run Prepare release again.",
            release.sha,
            release.version,
            fragments.join("\n  ")
        ));
    }
    Ok(Some(release))
}

/// Changelog fragment paths in `changelog.d/` at `rev`.
fn fragments_at(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    Ok(git(root, &["ls-tree", "--name-only", rev, "changelog.d/"])?
        .lines()
        .filter(|path| *path != "changelog.d/README.md")
        .map(str::to_owned)
        .collect())
}

/// Print the release commit in `before..HEAD` as `sha=` and `version=` lines for
/// `$GITHUB_OUTPUT`. Prints nothing if no commit raises the version.
pub fn release_commit(root: &Path, before: &str) -> Result<(), String> {
    match find_release_commit(root, before)? {
        Some(ReleaseCommit { sha, version }) => {
            eprintln!("Release commit {sha} raises the version to {version}.");
            println!("sha={sha}");
            println!("version={version}");
        }
        None => eprintln!("No commit since {before} raises the version, nothing to release."),
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

        Merging this PR tags its commit as `v{version}` and publishes the crate to crates.io.

        The merge queue rejects this PR if a PR with a changelog fragment merged after this PR was prepared. In that case, close this PR, delete its branch, and run Prepare release again."
    };
    if let Some(issue) = issue.filter(|issue| !issue.is_empty()) {
        write!(body, "\n\nRelated issue: {issue}").expect("writing to a String cannot fail");
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Repo(tempfile::TempDir);

    impl Repo {
        fn new() -> Self {
            let repo = Self(tempfile::tempdir().unwrap());
            repo.git(&["init", "--quiet", "-b", "test"]);
            repo.git(&["config", "user.email", "test@test.com"]);
            repo.git(&["config", "user.name", "Test"]);
            repo.git(&["config", "commit.gpgsign", "false"]);
            fs::create_dir(repo.path().join("changelog.d")).unwrap();
            repo.write("changelog.d/README.md", "# Changelog fragments");
            repo.set_version("1.0.0");
            repo.commit("initial");
            repo
        }

        fn path(&self) -> &Path {
            self.0.path()
        }

        fn git(&self, args: &[&str]) -> String {
            git(self.path(), args).unwrap()
        }

        fn write(&self, path: &str, content: &str) {
            fs::write(self.path().join(path), content).unwrap();
        }

        fn set_version(&self, version: &str) {
            self.write(
                "Cargo.toml",
                &format!("[package]\nname = \"test\"\nversion = \"{version}\"\n"),
            );
        }

        fn commit(&self, message: &str) -> String {
            self.git(&["add", "--all"]);
            self.git(&["commit", "--quiet", "-m", message]);
            self.head()
        }

        fn head(&self) -> String {
            self.git(&["rev-parse", "HEAD"]).trim().to_owned()
        }

        /// A regular PR that adds a changelog fragment.
        fn merge_feature(&self, name: &str) -> String {
            self.write(
                &format!("changelog.d/{name}.feature.md"),
                "A feature.\n\nauthors: alice",
            );
            self.commit(&format!("feat: {name}"))
        }

        /// A release PR that bumps the version and moves every fragment it knows about.
        fn merge_release(&self, version: &str, fragments: &[&str]) -> String {
            self.set_version(version);
            for name in fragments {
                fs::remove_file(self.path().join(format!("changelog.d/{name}.feature.md")))
                    .unwrap();
            }
            self.commit(&format!("chore(releasing): prepare v{version}"))
        }

        fn find(&self, before: &str) -> Result<Option<ReleaseCommit>, String> {
            find_release_commit(self.path(), before)
        }
    }

    fn release(sha: &str, version: &str) -> ReleaseCommit {
        ReleaseCommit {
            sha: sha.to_owned(),
            version: semver::Version::parse(version).unwrap(),
        }
    }

    #[test]
    fn push_without_version_change_is_not_a_release() {
        let repo = Repo::new();
        let before = repo.head();
        repo.write(
            "Cargo.toml",
            "[package]\nname = \"test\"\nversion = \"1.0.0\"\n# dep bump\n",
        );
        repo.commit("chore(deps): bump");
        repo.merge_feature("a");

        assert_eq!(repo.find(&before).unwrap(), None);
    }

    #[test]
    fn release_alone_in_push() {
        let repo = Repo::new();
        repo.merge_feature("a");
        let before = repo.head();
        let sha = repo.merge_release("1.1.0", &["a"]);

        assert_eq!(repo.find(&before).unwrap(), Some(release(&sha, "1.1.0")));
    }

    #[test]
    fn release_followed_by_another_pr_in_the_same_push_tags_the_release_commit() {
        let repo = Repo::new();
        repo.merge_feature("a");
        let before = repo.head();
        let sha = repo.merge_release("1.1.0", &["a"]);
        repo.merge_feature("b");

        assert_ne!(repo.head(), sha);
        assert_eq!(repo.find(&before).unwrap(), Some(release(&sha, "1.1.0")));
    }

    #[test]
    fn pr_merged_after_preparation_fails_the_release() {
        let repo = Repo::new();
        repo.merge_feature("a");
        let before = repo.head();
        // "b" merges between preparation and the release merge, in the same push or earlier.
        repo.merge_feature("b");
        repo.merge_release("1.1.0", &["a"]);

        let err = repo.find(&before).unwrap_err();
        assert!(err.contains("changelog.d/b.feature.md"), "{err}");
        assert!(!err.contains("changelog.d/README.md"), "{err}");
    }

    #[test]
    fn two_version_bumps_in_one_push_fail() {
        let repo = Repo::new();
        let before = repo.head();
        repo.merge_release("1.1.0", &[]);
        repo.merge_release("1.2.0", &[]);

        let err = repo.find(&before).unwrap_err();
        assert!(err.contains("More than one commit"), "{err}");
    }
}
