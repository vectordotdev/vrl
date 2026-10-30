mod breaking;

use breaking::BreakingSections;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const FRAGMENT_TYPES: &[(&str, &str)] = &[
    ("breaking", "Breaking Changes & Upgrade Guide"),
    ("security", "Security"),
    ("deprecation", "Deprecations"),
    ("feature", "New Features"),
    ("enhancement", "Enhancements"),
    ("fix", "Fixes"),
];

const CHANGELOG_MARKER: &str = "<!-- changelog start -->\n";

#[derive(Debug)]
#[allow(clippy::struct_field_names)] // `fragment_type` distinguishes it from its content.
struct Fragment {
    pr_numbers: Vec<u64>,
    fragment_type: String,
    content: String,
    authors: Vec<String>,
    breaking: Option<BreakingSections>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PullRequestMetadata {
    Optional,
    Required,
}

/// Parse Vector's final `authors: <name> [<name> ...]` line for every fragment type.
fn parse_authors(raw: &str) -> Result<(String, Vec<String>), String> {
    let raw = raw.strip_suffix('\n').unwrap_or(raw);
    let (body, last) = raw.rsplit_once('\n').unwrap_or(("", raw));
    let names = last
        .strip_prefix("authors: ")
        .ok_or("last line must be 'authors: <name> [<name> ...]' (no trailing blank lines)")?;
    if names.trim().is_empty() {
        return Err("authors line must list at least one GitHub username".into());
    }
    if names.contains('@') || names.contains(',') {
        return Err("authors must be space-delimited GitHub usernames without '@'".into());
    }
    let authors: Vec<String> = names.split_whitespace().map(str::to_string).collect();
    if authors.iter().any(|name| name == "TODO_your_gh_handle") {
        return Err("replace the authors placeholder with a real GitHub username".into());
    }
    Ok((body.to_string(), authors))
}

fn format_authors(authors: &[String]) -> String {
    authors
        .iter()
        .map(|a| format!("[{a}](https://github.com/{a})"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_pull_requests(pr_numbers: &[u64]) -> String {
    pr_numbers
        .iter()
        .map(|pr| format!("[#{pr}](https://github.com/vectordotdev/vrl/pull/{pr})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Validate a fragment filename, returning `(description, fragment_type)`.
fn validate_fragment_filename(filename: &str) -> Result<(&str, &str), String> {
    let parts: Vec<&str> = filename.splitn(3, '.').collect();
    if parts.len() != 3 || parts[2] != "md" {
        return Err(format!(
            "Invalid fragment filename '{filename}': expected '<description>.<type>.md'"
        ));
    }

    if parts[0].is_empty() {
        return Err(format!(
            "Invalid fragment filename '{filename}': first segment must describe the change"
        ));
    }

    let valid_types: Vec<&str> = FRAGMENT_TYPES.iter().map(|(t, _)| *t).collect();
    let type_lower = parts[1].to_ascii_lowercase();
    if !valid_types.contains(&type_lower.as_str()) {
        return Err(format!(
            "Invalid fragment type '{}' in '{filename}'. Valid types: {}",
            parts[1],
            valid_types.join(", ")
        ));
    }

    Ok((parts[0], parts[1]))
}

pub struct Changelog {
    repo_root: PathBuf,
}

impl Changelog {
    pub fn new(repo_root: &Path) -> Self {
        Self {
            repo_root: repo_root.to_path_buf(),
        }
    }

    fn changelog_dir(&self) -> PathBuf {
        self.repo_root.join("changelog.d")
    }

    fn parse_fragment(path: &Path) -> Result<Fragment, String> {
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| format!("Invalid fragment path: {}", path.display()))?;

        let (_, fragment_type) = validate_fragment_filename(filename)?;

        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

        let is_breaking = fragment_type.eq_ignore_ascii_case("breaking");
        let (content, authors) = parse_authors(&raw.replace("\r\n", "\n"))
            .map_err(|e| format!("{e} (in {})", path.display()))?;
        let content = if is_breaking {
            content
        } else {
            content.trim().to_string()
        };
        let breaking = is_breaking
            .then(|| BreakingSections::parse(&content))
            .transpose()
            .map_err(|e| format!("{e} (in {})", path.display()))?;

        Ok(Fragment {
            pr_numbers: Vec::new(),
            fragment_type: fragment_type.to_ascii_lowercase(),
            content,
            authors,
            breaking,
        })
    }

    fn collect_fragments(
        &self,
        pull_request_metadata: PullRequestMetadata,
    ) -> Result<BTreeMap<String, Vec<Fragment>>, String> {
        if pull_request_metadata == PullRequestMetadata::Required {
            self.ensure_complete_history()?;
        }

        self.validate_breaking_anchors()?;
        let mut grouped: BTreeMap<String, Vec<Fragment>> = BTreeMap::new();

        for entry in Self::read_fragment_dir(&self.changelog_dir())? {
            let mut fragment = Self::parse_fragment(&entry)?;
            fragment.pr_numbers =
                lookup_pull_requests(&self.repo_root, &entry, pull_request_metadata)?;
            grouped
                .entry(fragment.fragment_type.clone())
                .or_default()
                .push(fragment);
        }

        if grouped.is_empty() {
            return Err("No changelog fragments found in changelog.d/".to_string());
        }

        Ok(grouped)
    }

    // Test builds skip the Git history check but retain the production signature.
    #[cfg_attr(test, allow(clippy::unused_self, clippy::unnecessary_wraps))]
    fn ensure_complete_history(&self) -> Result<(), String> {
        #[cfg(not(test))]
        {
            let is_shallow = run_command(
                "git",
                ["rev-parse", "--is-shallow-repository"],
                &self.repo_root,
            )?;
            if is_shallow.trim() == "true" {
                return Err(
                    "Cannot resolve changelog PRs from a shallow repository. Fetch the full history and retry."
                        .to_string(),
                );
            }
        }

        Ok(())
    }

    /// List fragment file paths in changelog.d/, excluding README.md and non-files.
    fn read_fragment_dir(dir: &Path) -> Result<Vec<PathBuf>, String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;

        let mut paths = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|e| format!("Failed to read directory entry: {e}"))?
                .path();

            if !path.is_file() {
                continue;
            }
            if path.file_name().and_then(|f| f.to_str()) == Some("README.md") {
                continue;
            }
            paths.push(path);
        }
        Ok(paths)
    }

    fn validate_breaking_anchors(&self) -> Result<(), String> {
        let mut seen = BTreeMap::new();
        for path in Self::read_fragment_dir(&self.changelog_dir())? {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !validate_fragment_filename(filename)
                .is_ok_and(|(_, kind)| kind.eq_ignore_ascii_case("breaking"))
            {
                continue;
            }
            let fragment = Self::parse_fragment(&path)?;
            if let Some(sections) = fragment.breaking
                && let Some(other) = seen.insert(sections.anchor.clone(), path.clone())
            {
                return Err(format!(
                    "duplicate upgrade-guide anchor '#{}' in {} and {}",
                    sections.anchor,
                    other.display(),
                    path.display()
                ));
            }
        }
        Ok(())
    }

    /// Indent continuation lines of multiline content so they stay inside
    /// the markdown list item (2-space indent to align with `- ` prefix).
    fn indent_continuation(text: &str) -> String {
        let mut lines = text.lines();
        let mut result = lines.next().unwrap_or("").to_string();
        for line in lines {
            result.push('\n');
            if !line.is_empty() {
                result.push_str("  ");
                result.push_str(line);
            }
        }
        result
    }

    fn render_section(
        grouped: &BTreeMap<String, Vec<Fragment>>,
        version: &semver::Version,
        date: &str,
    ) -> String {
        let tag_url = format!("https://github.com/vectordotdev/vrl/releases/tag/v{version}");
        let mut section = format!("## [{version} ({date})]({tag_url})\n");

        for (type_key, type_heading) in FRAGMENT_TYPES {
            if let Some(fragments) = grouped.get(*type_key) {
                writeln!(section, "\n### {type_heading}\n")
                    .expect("writing to a String cannot fail");
                for fragment in fragments {
                    let authors = format_authors(&fragment.authors);
                    let attribution = if fragment.pr_numbers.is_empty() {
                        format!("Thanks to {authors} for contributing this change!")
                    } else {
                        let pull_requests = format_pull_requests(&fragment.pr_numbers);
                        let label = if fragment.pr_numbers.len() == 1 {
                            "PR"
                        } else {
                            "PRs"
                        };
                        format!("Thanks to {authors} for contributing {label} {pull_requests}!")
                    };
                    let breaking_content = fragment
                        .breaking
                        .as_ref()
                        .map(|breaking| breaking.render(version));
                    let content = breaking_content.as_deref().unwrap_or(&fragment.content);
                    let indented = Self::indent_continuation(content);
                    writeln!(section, "- {indented}\n\n  *{attribution}*")
                        .expect("writing to a String cannot fail");
                }
            }
        }

        section
    }

    pub fn generate_section(
        &self,
        version: &semver::Version,
        pull_request_metadata: PullRequestMetadata,
    ) -> Result<String, String> {
        let grouped = self.collect_fragments(pull_request_metadata)?;
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        Ok(Self::render_section(&grouped, version, &date))
    }

    /// Insert a pre-generated changelog section into CHANGELOG.md and remove fragments.
    pub fn apply_section(&self, version: &semver::Version, section: &str) -> Result<(), String> {
        let changelog_path = self.repo_root.join("CHANGELOG.md");
        let content = std::fs::read_to_string(&changelog_path)
            .map_err(|e| format!("Failed to read CHANGELOG.md: {e}"))?;

        let new_content = Self::insert_section(&content, section)?;
        std::fs::write(&changelog_path, new_content)
            .map_err(|e| format!("Failed to write CHANGELOG.md: {e}"))?;
        println!("Updated CHANGELOG.md with {version} section.");

        for path in Self::read_fragment_dir(&self.changelog_dir())? {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        }
        println!("Removed changelog fragments.");
        Ok(())
    }

    fn insert_section(content: &str, section: &str) -> Result<String, String> {
        let marker_pos = content
            .find(CHANGELOG_MARKER)
            .ok_or("Could not find '<!-- changelog start -->' marker in CHANGELOG.md")?;

        let insert_pos = marker_pos + CHANGELOG_MARKER.len();
        let mut new_content = String::with_capacity(content.len() + section.len() + 2);
        new_content.push_str(&content[..insert_pos]);
        new_content.push('\n');
        new_content.push_str(section);
        new_content.push('\n');
        new_content.push_str(&content[insert_pos..]);

        Ok(new_content)
    }

    fn changed_fragments(&self, filter: &str) -> Result<Vec<PathBuf>, String> {
        let output = run_command(
            "git",
            [
                "diff",
                "--name-only",
                &format!("--diff-filter={filter}"),
                "--merge-base",
                "origin/main",
                "--",
                "changelog.d",
            ],
            &self.repo_root,
        )?;
        Ok(output
            .lines()
            .map(PathBuf::from)
            .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("README.md"))
            .collect())
    }

    /// Require a new fragment and validate every added, modified, or renamed fragment.
    pub fn check_fragments(&self) -> Result<(), String> {
        let filenames = self.changed_fragments("A")?;

        if filenames.is_empty() {
            return Err(indoc::indoc! {"
                No changelog fragments detected.
                If no changes necessitate user-facing explanations, add the GH label 'no-changelog'.
                Otherwise, add changelog fragments to changelog.d/
                For details, see 'changelog.d/README.md'"}
            .to_string());
        }

        for path in self.changed_fragments("AMR")? {
            let filename = path
                .file_name()
                .and_then(|f| f.to_str())
                .ok_or_else(|| format!("Invalid fragment path: {}", path.display()))?;

            println!("validating '{filename}'");
            validate_fragment_filename(filename)?;
            let full_path = self.repo_root.join(path);
            Self::parse_fragment(&full_path)?;
        }

        self.validate_breaking_anchors()?;
        println!("changelog additions are valid.");
        Ok(())
    }
}

/// Find every PR that added, edited, or renamed the current lifetime of a
/// changelog fragment from each commit's `... (#12345)` title.
#[cfg(not(test))]
fn lookup_pull_requests(
    repo_root: &Path,
    fragment_path: &Path,
    pull_request_metadata: PullRequestMetadata,
) -> Result<Vec<u64>, String> {
    lookup_pull_requests_from_git(repo_root, fragment_path, pull_request_metadata)
}

fn lookup_pull_requests_from_git(
    repo_root: &Path,
    fragment_path: &Path,
    pull_request_metadata: PullRequestMetadata,
) -> Result<Vec<u64>, String> {
    let relative_path = fragment_path.strip_prefix(repo_root).map_err(|_| {
        format!(
            "Fragment path {} is outside the repository root {}",
            fragment_path.display(),
            repo_root.display()
        )
    })?;
    let relative_path = relative_path.to_str().ok_or_else(|| {
        format!(
            "Fragment path is not valid UTF-8: {}",
            fragment_path.display()
        )
    })?;

    let addition_commits = run_command(
        "git",
        [
            "log",
            "--follow",
            "--format=%H",
            "--diff-filter=A",
            "--",
            relative_path,
        ],
        repo_root,
    )?;
    let Some(latest_addition) = addition_commits.lines().next() else {
        return match pull_request_metadata {
            PullRequestMetadata::Optional => Ok(Vec::new()),
            PullRequestMetadata::Required => Err(format!(
                "Could not find the commit that added {relative_path}; cannot determine its pull requests."
            )),
        };
    };

    let commit_history = run_command(
        "git",
        [
            "log",
            "--follow",
            "--format=%H%x09%s",
            "--diff-filter=AMR",
            "--",
            relative_path,
        ],
        repo_root,
    )?;

    parse_pull_request_history(&commit_history, latest_addition, pull_request_metadata).map_err(
        |error| {
            format!("Could not determine every PR that added or edited {relative_path}: {error}")
        },
    )
}

#[cfg(test)]
#[allow(clippy::unnecessary_wraps)] // Matches the production implementation's signature.
fn lookup_pull_requests(_: &Path, _: &Path, _: PullRequestMetadata) -> Result<Vec<u64>, String> {
    Ok(vec![42])
}

fn run_command<const N: usize>(cmd: &str, args: [&str; N], cwd: &Path) -> Result<String, String> {
    let display = format!("{cmd} {}", args.join(" "));
    let output = std::process::Command::new(cmd)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("Failed to run `{display}`: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "`{display}` failed (exit {}):\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_pull_request_history(
    commit_history: &str,
    latest_addition: &str,
    pull_request_metadata: PullRequestMetadata,
) -> Result<Vec<u64>, String> {
    let mut numbers = Vec::new();
    for line in commit_history
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let (commit, title) = line
            .split_once('\t')
            .ok_or_else(|| format!("Malformed git log entry `{line}`"))?;

        match parse_pull_request_number(title) {
            Ok(number) if !numbers.contains(&number) => numbers.push(number),
            Ok(_) => {}
            Err(_) if pull_request_metadata == PullRequestMetadata::Optional => {}
            Err(error) => return Err(format!("Commit {commit} `{title}`: {error}")),
        }

        if commit == latest_addition {
            return Ok(numbers);
        }
    }

    match pull_request_metadata {
        PullRequestMetadata::Optional => Ok(Vec::new()),
        PullRequestMetadata::Required => Err(format!(
            "Could not find latest addition commit {latest_addition} in the fragment history"
        )),
    }
}

fn parse_pull_request_number(commit_title: &str) -> Result<u64, String> {
    let number = commit_title
        .trim()
        .strip_suffix(')')
        .and_then(|title| title.rsplit_once("(#"))
        .map(|(_, number)| number)
        .filter(|number| !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| "Commit title must end with `(#<PR number>)`".to_string())?;

    number
        .parse()
        .map_err(|error| format!("Invalid PR number in commit title `{commit_title}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;

    fn setup_test_repo(fragments: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let changelog_dir = dir.path().join("changelog.d");
        fs::create_dir(&changelog_dir).unwrap();
        fs::write(changelog_dir.join("README.md"), "# Changelog fragments").unwrap();

        for (name, content) in fragments {
            fs::write(changelog_dir.join(name), content).unwrap();
        }

        fs::write(
            dir.path().join("CHANGELOG.md"),
            indoc! {"
                # Changelog

                <!-- changelog start -->

                ## [0.31.0 (2026-03-05)]

                ### Fixes

                - Some old fix.

                  (https://github.com/vectordotdev/vrl/pull/100)
            "},
        )
        .unwrap();

        dir
    }

    #[test]
    fn parses_current_fragment_lifetime_pull_requests() {
        let history = indoc! {"
            current-edit	fix(foo): adjust entry (#300)
            current-add	feat(foo): add entry (#298)
            old-edit	fix(foo): old adjustment (#120)
            old-add	feat(foo): old addition (#119)
        "};

        assert_eq!(
            parse_pull_request_history(history, "current-add", PullRequestMetadata::Required)
                .unwrap(),
            vec![300, 298]
        );
    }

    #[test]
    fn optional_pull_request_metadata_skips_unmerged_commits() {
        let history = indoc! {"
            edit	fix(foo): adjust entry
            add	feat(foo): add entry (#123)
        "};

        assert_eq!(
            parse_pull_request_history(history, "add", PullRequestMetadata::Optional).unwrap(),
            vec![123]
        );
        assert!(parse_pull_request_history(history, "add", PullRequestMetadata::Required).is_err());
    }

    #[test]
    fn pull_request_lookup_handles_renames_and_uncommitted_fragments() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        let changelog_dir = repo.join("changelog.d");
        fs::create_dir(&changelog_dir).unwrap();

        run_command("git", ["init", "--quiet", "-b", "test"], repo).unwrap();
        run_command("git", ["config", "core.hooksPath", "/dev/null"], repo).unwrap();
        run_command("git", ["config", "user.name", "VRL Test"], repo).unwrap();
        run_command("git", ["config", "user.email", "vrl@example.com"], repo).unwrap();
        run_command("git", ["config", "commit.gpgsign", "false"], repo).unwrap();

        let original = changelog_dir.join("original.enhancement.md");
        let renamed = changelog_dir.join("renamed.enhancement.md");
        fs::write(
            &original,
            "Original entry.\nIt documents the first behavior.\nIt includes more detail.\n",
        )
        .unwrap();
        run_command("git", ["add", "changelog.d/original.enhancement.md"], repo).unwrap();
        run_command(
            "git",
            ["commit", "--quiet", "-m", "feat(foo): add entry (#100)"],
            repo,
        )
        .unwrap();

        run_command(
            "git",
            [
                "mv",
                "changelog.d/original.enhancement.md",
                "changelog.d/renamed.enhancement.md",
            ],
            repo,
        )
        .unwrap();
        fs::write(
            &renamed,
            "Original entry.\nIt documents the first behavior.\nIt includes more detail.\nOne extra detail.\n",
        )
        .unwrap();
        run_command("git", ["add", "changelog.d/renamed.enhancement.md"], repo).unwrap();
        run_command(
            "git",
            [
                "commit",
                "--quiet",
                "-m",
                "fix(foo): rename and edit entry (#101)",
            ],
            repo,
        )
        .unwrap();

        assert_eq!(
            lookup_pull_requests_from_git(repo, &renamed, PullRequestMetadata::Required).unwrap(),
            vec![101, 100]
        );

        let uncommitted = changelog_dir.join("uncommitted.fix.md");
        fs::write(&uncommitted, "Uncommitted entry.\n").unwrap();
        assert_eq!(
            lookup_pull_requests_from_git(repo, &uncommitted, PullRequestMetadata::Optional)
                .unwrap(),
            Vec::<u64>::new()
        );
        assert!(
            lookup_pull_requests_from_git(repo, &uncommitted, PullRequestMetadata::Required)
                .is_err()
        );
    }

    // --- validate_fragment_filename ---

    #[test]
    fn valid_fragment_filenames() {
        let (description, ty) = validate_fragment_filename("add-feature.feature.md").unwrap();
        assert_eq!(description, "add-feature");
        assert_eq!(ty, "feature");

        for (ty, _) in FRAGMENT_TYPES {
            validate_fragment_filename(&format!("description.{ty}.md")).unwrap();
        }

        let (_, ty) = validate_fragment_filename("description.Fix.md").unwrap();
        assert_eq!(ty, "Fix"); // raw value; normalized to lowercase in parse_fragment

        let (description, _) = validate_fragment_filename("improve-docs.feature.md").unwrap();
        assert_eq!(description, "improve-docs");
    }

    #[test]
    fn invalid_fragment_filenames() {
        for (filename, expected) in [
            (".feature.md", "must describe the change"),
            ("description.unknown.md", "Invalid fragment type 'unknown'"),
            (
                "description.feature.txt",
                "expected '<description>.<type>.md'",
            ),
            ("description.md", "expected '<description>.<type>.md'"),
        ] {
            let err = validate_fragment_filename(filename).unwrap_err();
            assert!(err.contains(expected), "{err}");
        }
    }

    // --- parse_authors ---

    #[test]
    fn parse_authors_single() {
        let (desc, authors) = parse_authors("Fixed a bug.\n\nauthors: alice").unwrap();
        assert_eq!(desc, "Fixed a bug.\n");
        assert_eq!(authors, vec!["alice"]);
    }

    #[test]
    fn parse_authors_multiple() {
        for suffix in ["", "\n"] {
            let (desc, authors) =
                parse_authors(&format!("A feature.\n\nauthors: alice bob{suffix}")).unwrap();
            assert_eq!(desc, "A feature.\n");
            assert_eq!(authors, vec!["alice", "bob"]);
        }
    }

    #[test]
    fn parse_authors_rejects_non_vector_formats() {
        for invalid in [
            "Fixed a bug.",
            "Fixed a bug.\n\nauthor: alice",
            "Fixed a bug.\n\nAuthors: alice",
            "Fixed a bug.\n\nauthors: @alice @bob",
            "Fixed a bug.\n\nauthors: alice, bob",
            "Fixed a bug.\n\nauthors: TODO_your_gh_handle",
            "Fixed a bug.\n\nauthors: alice\n\n",
        ] {
            assert!(parse_authors(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn parse_authors_empty_value() {
        let err = parse_authors("Fixed a bug.\n\nauthors: ").unwrap_err();
        assert!(err.contains("at least one"), "{err}");
    }

    // --- parse_fragment ---

    #[test]
    fn parse_reads_content_and_authors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fix-bug.fix.md");
        fs::write(&path, "Fixed a bug.\n\nauthors: alice\n").unwrap();

        let fragment = Changelog::parse_fragment(&path).unwrap();
        assert!(fragment.pr_numbers.is_empty());
        assert_eq!(fragment.fragment_type, "fix");
        assert_eq!(fragment.content, "Fixed a bug.");
        assert_eq!(fragment.authors, vec!["alice"]);
    }

    #[test]
    fn parse_fragment_missing_authors_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fix-bug.fix.md");
        fs::write(&path, "Fixed a bug.\n").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("authors:"), "{err}");
    }

    const BREAKING_FRAGMENT: &str = indoc! {r#"
        # Fallible timestamp conversion {#timestamp-conversion}

        ## Summary

        Nanosecond conversion can fail.
        Handle the error even when the timestamp usually fits.

        ## Migration

        Handle the error:

        #### Old

        ```vrl
        # Keep this VRL comment unchanged.
        to_unix_timestamp(ts, unit: "nanoseconds")
        ```

        #### New

        ```vrl
        to_unix_timestamp!(ts, unit: "nanoseconds")
        ```

        authors: alice bob
    "#};

    #[test]
    fn breaking_fragment_preserves_sections_examples_and_credit() {
        let dir = setup_test_repo(&[("timestamp.breaking.md", BREAKING_FRAGMENT)]);
        let section = Changelog::new(dir.path())
            .generate_section(
                &semver::Version::new(1, 0, 0),
                PullRequestMetadata::Optional,
            )
            .unwrap();
        for expected in [
            "- <a id=\"v1.0.0-timestamp-conversion\"></a> **Fallible timestamp conversion**",
            "  **Summary**\n\n  Nanosecond conversion can fail.\n  Handle the error even when the timestamp usually fits.",
            "  **Migration**\n\n  Handle the error:",
            "  #### Old\n\n  ```vrl\n  # Keep this VRL comment unchanged.",
            "  #### New\n\n  ```vrl\n  to_unix_timestamp!(ts, unit: \"nanoseconds\")",
            "Thanks to [alice](https://github.com/alice), [bob](https://github.com/bob)",
        ] {
            assert!(
                section.contains(expected),
                "missing {expected:?} in {section}"
            );
        }
    }

    #[test]
    fn breaking_schema_matches_vector() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("change.breaking.md");
        let minimal =
            "# A change\n\n## Summary\n\nChanged behavior.\n\n## Migration\n\nauthors: alice\n";
        // No examples, an empty Migration, a derived anchor, and CRLF are valid in Vector.
        for valid in [minimal.to_string(), minimal.replace('\n', "\r\n")] {
            fs::write(&path, valid).unwrap();
            let fragment = Changelog::parse_fragment(&path).unwrap();
            assert_eq!(fragment.breaking.unwrap().anchor, "a-change");
        }
        for invalid in [
            minimal.replacen("# A change", "A change", 1),
            minimal.replace("# A change", "# "),
            minimal.replace("# A change", "# TODO title"),
            minimal.replace("# A change", "# A change {#Invalid}"),
            minimal.replace("# A change", "# Upgrade guide"),
            minimal.replace("## Summary", "Prose before the summary.\n\n## Summary"),
            minimal.replace("## Summary", "## Summary\n\n## Summary"),
            minimal.replace("## Summary", "## Details"),
            minimal.replace("Changed behavior.", ""),
            minimal.replace("Changed behavior.", "TODO describe the change"),
            minimal.replace("## Migration\n", "## Migration\n\nTODO "),
            minimal.replace("## Migration", "## Summary"),
            minimal
                .replace("## Summary", "## Swap")
                .replace("## Migration", "## Summary")
                .replace("## Swap", "## Migration"),
            minimal.replace("authors: alice", "authors: @alice"),
            minimal.replace("authors: alice", "authors: alice, bob"),
            minimal.replace("authors: alice", "authors: TODO_your_gh_handle"),
            minimal.replace("authors: alice", "Authors: alice"),
            format!("{minimal}\n"),
            format!("\n{minimal}"),
        ] {
            fs::write(&path, &invalid).unwrap();
            assert!(
                Changelog::parse_fragment(&path).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn breaking_anchors_must_be_unique() {
        let dir = setup_test_repo(&[
            ("first.breaking.md", BREAKING_FRAGMENT),
            ("second.breaking.md", BREAKING_FRAGMENT),
        ]);
        let error = Changelog::new(dir.path())
            .generate_section(
                &semver::Version::new(1, 0, 0),
                PullRequestMetadata::Optional,
            )
            .unwrap_err();
        assert!(
            error.contains("duplicate upgrade-guide anchor '#timestamp-conversion'"),
            "{error}"
        );
    }

    #[test]
    fn every_fragment_type_uses_vector_authors() {
        let dir = tempfile::tempdir().unwrap();
        for (kind, _) in FRAGMENT_TYPES {
            let path = dir.path().join(format!("change.{kind}.md"));
            let content = if *kind == "breaking" {
                BREAKING_FRAGMENT
            } else {
                "A change.\n\nauthors: alice bob\n"
            };
            for valid in [content.to_string(), content.replace('\n', "\r\n")] {
                fs::write(&path, valid).unwrap();
                let fragment = Changelog::parse_fragment(&path).unwrap();
                assert_eq!(fragment.authors, vec!["alice", "bob"], "{kind}");
            }
            fs::write(&path, content.replace("alice bob", "alice, bob")).unwrap();
            assert!(
                Changelog::parse_fragment(&path).is_err(),
                "accepted commas in {kind}"
            );
        }
    }

    // --- collect_fragments ---

    #[test]
    fn groups_by_type() {
        let dir = setup_test_repo(&[
            ("feature-a.feature.md", "Feature A\n\nauthors: alice"),
            ("feature-b.feature.md", "Feature B\n\nauthors: bob"),
            ("bug-fix.fix.md", "Bug fix\n\nauthors: carol"),
        ]);
        let grouped = Changelog::new(dir.path())
            .collect_fragments(PullRequestMetadata::Optional)
            .unwrap();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["feature"].len(), 2);
        assert_eq!(grouped["fix"].len(), 1);
    }

    #[test]
    fn skips_readme() {
        let dir = setup_test_repo(&[("feature.feature.md", "A feature\n\nauthors: alice")]);
        let grouped = Changelog::new(dir.path())
            .collect_fragments(PullRequestMetadata::Optional)
            .unwrap();

        assert_eq!(grouped.len(), 1);
        assert!(!grouped.contains_key("README"));
    }

    #[test]
    fn errors_when_empty() {
        let dir = setup_test_repo(&[]);
        let err = Changelog::new(dir.path())
            .collect_fragments(PullRequestMetadata::Optional)
            .unwrap_err();
        assert!(err.contains("No changelog fragments found"), "{err}");
    }

    // --- check_fragments ---

    fn run_git(repo: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    fn setup_git_check_repo(fragments: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();

        // Use a non-protected branch name so local git hooks don't block commits.
        // The branch name doesn't matter — only refs/remotes/origin/main is used by check_fragments.
        for cmd in [
            vec!["init", "-b", "base"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "Test"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "core.fsmonitor", "false"],
            vec!["config", "core.untrackedCache", "false"],
        ] {
            run_git(repo, &cmd);
        }

        let changelog_dir = repo.join("changelog.d");
        fs::create_dir(&changelog_dir).unwrap();
        fs::write(changelog_dir.join("README.md"), "# Changelog fragments").unwrap();

        run_git(repo, &["add", "-A"]);
        run_git(repo, &["commit", "-m", "initial"]);
        run_git(repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);

        for (name, content) in fragments {
            fs::write(changelog_dir.join(name), content).unwrap();
        }

        run_git(repo, &["add", "-A"]);
        run_git(repo, &["commit", "-m", "add fragments"]);

        dir
    }

    #[test]
    fn check_fragments_valid_passes() {
        let dir =
            setup_git_check_repo(&[("fix-something.fix.md", "Fixed something.\n\nauthors: alice")]);
        let result = Changelog::new(dir.path()).check_fragments();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn check_fragments_missing_authors_fails() {
        let dir = setup_git_check_repo(&[("fix-something.fix.md", "Fixed something.\n")]);
        let err = Changelog::new(dir.path()).check_fragments().unwrap_err();
        assert!(err.contains("authors"), "{err}");
    }

    #[test]
    fn check_fragments_validates_modified_breaking_fragments() {
        let dir = setup_git_check_repo(&[("timestamp.breaking.md", BREAKING_FRAGMENT)]);
        let repo = dir.path();
        run_git(repo, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        fs::write(
            repo.join("changelog.d/new.fix.md"),
            "A fix.\n\nauthors: alice\n",
        )
        .unwrap();
        fs::write(
            repo.join("changelog.d/timestamp.breaking.md"),
            "Unstructured change.\n\nauthors: alice\n",
        )
        .unwrap();
        run_git(repo, &["add", "-A"]);
        run_git(
            repo,
            &["commit", "-m", "modify breaking fragment and add fix"],
        );
        let error = Changelog::new(repo).check_fragments().unwrap_err();
        assert!(error.contains("H1 title"), "{error}");
    }

    // --- render_section ---

    #[test]
    fn respects_type_ordering() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_numbers: vec![20],
                fragment_type: "fix".to_string(),
                content: "Fixed a bug".to_string(),
                authors: vec!["alice".to_string()],
                breaking: None,
            }],
        );
        grouped.insert(
            "breaking".to_string(),
            vec![Fragment {
                pr_numbers: vec![10],
                fragment_type: "breaking".to_string(),
                content: String::new(),
                authors: vec!["bob".to_string()],
                breaking: Some(BreakingSections::parse("# Removed old API\n\n## Summary\n\nOld API removed.\n\n## Migration\n\nUse the new API.").unwrap()),
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(1, 0, 0), "2026-04-16");

        let breaking_pos = section.find("Breaking Changes").unwrap();
        let fix_pos = section.find("Fixes").unwrap();
        assert!(breaking_pos < fix_pos);
    }

    #[test]
    fn skips_missing_types() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_numbers: vec![1],
                fragment_type: "fix".to_string(),
                content: "A fix".to_string(),
                authors: vec!["alice".to_string()],
                breaking: None,
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(0, 1, 0), "2026-01-01");

        assert!(section.contains("### Fixes"));
        assert!(!section.contains("### New Features"));
        assert!(!section.contains("### Breaking"));
    }

    #[test]
    fn section_format() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "feature".to_string(),
            vec![Fragment {
                pr_numbers: vec![42],
                fragment_type: "feature".to_string(),
                content: "Added something cool".to_string(),
                authors: vec!["alice".to_string()],
                breaking: None,
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(1, 2, 0), "2026-04-16");

        let expected = indoc! {"
            ## [1.2.0 (2026-04-16)](https://github.com/vectordotdev/vrl/releases/tag/v1.2.0)

            ### New Features

            - Added something cool

              *Thanks to [alice](https://github.com/alice) for contributing PR [#42](https://github.com/vectordotdev/vrl/pull/42)!*
        "};
        assert_eq!(section, expected);
    }

    #[test]
    fn section_format_multiple_authors() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_numbers: vec![7, 8],
                fragment_type: "fix".to_string(),
                content: "A fix".to_string(),
                authors: vec!["alice".to_string(), "bob".to_string()],
                breaking: None,
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(0, 1, 0), "2026-01-01");

        assert!(section.contains(
            "*Thanks to [alice](https://github.com/alice), [bob](https://github.com/bob) for contributing PRs [#7](https://github.com/vectordotdev/vrl/pull/7), [#8](https://github.com/vectordotdev/vrl/pull/8)!*"
        ), "{section}");
    }

    #[test]
    fn section_format_without_pull_request() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_numbers: Vec::new(),
                fragment_type: "fix".to_string(),
                content: "An unmerged fix".to_string(),
                authors: vec!["alice".to_string()],
                breaking: None,
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(0, 1, 0), "2026-01-01");

        assert!(
            section.contains(
                "*Thanks to [alice](https://github.com/alice) for contributing this change!*"
            ),
            "{section}"
        );
    }

    #[test]
    fn multiline_fragment_indents_continuation() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "enhancement".to_string(),
            vec![Fragment {
                pr_numbers: vec![99],
                fragment_type: "enhancement".to_string(),
                content: "Improved the API.\n\nUse `bar()` to enable the new behavior.".to_string(),
                authors: vec!["alice".to_string()],
                breaking: None,
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(2, 0, 0), "2026-04-17");

        let expected = indoc! {"
            ## [2.0.0 (2026-04-17)](https://github.com/vectordotdev/vrl/releases/tag/v2.0.0)

            ### Enhancements

            - Improved the API.

              Use `bar()` to enable the new behavior.

              *Thanks to [alice](https://github.com/alice) for contributing PR [#99](https://github.com/vectordotdev/vrl/pull/99)!*
        "};
        assert_eq!(section, expected);
    }

    // --- insert_section ---

    #[test]
    fn inserts_after_marker() {
        let content = indoc! {"
            # Changelog

            <!-- changelog start -->

            ## [0.1.0 (2025-01-01)]
        "};

        let result = Changelog::insert_section(content, "## [1.0.0 (2026-04-16)]\n").unwrap();

        let new_pos = result.find("## [1.0.0").unwrap();
        let old_pos = result.find("## [0.1.0").unwrap();
        assert!(new_pos < old_pos);
    }

    #[test]
    fn errors_without_marker() {
        let err = Changelog::insert_section("# Changelog\n", "## [1.0.0]\n").unwrap_err();
        assert!(err.contains("marker"), "{err}");
    }

    // --- apply_section (integration) ---

    #[test]
    fn applies_section_and_removes_fragments() {
        let dir = setup_test_repo(&[
            ("new-feature.feature.md", "New feature\n\nauthors: alice"),
            ("bug-fix.fix.md", "Bug fix\n\nauthors: bob"),
        ]);

        let changelog = Changelog::new(dir.path());
        let version = semver::Version::new(1, 0, 0);
        let section = changelog
            .generate_section(&version, PullRequestMetadata::Optional)
            .unwrap();
        changelog.apply_section(&version, &section).unwrap();

        let content = fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap();
        assert!(content.contains("## [1.0.0"));
        assert!(content.contains("New feature"));
        assert!(content.contains("Bug fix"));

        let remaining: Vec<_> = fs::read_dir(dir.path().join("changelog.d"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(remaining, vec!["README.md"]);
    }
}
