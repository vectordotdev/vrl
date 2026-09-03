use std::collections::BTreeMap;
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
struct Fragment {
    pr_numbers: Vec<u64>,
    fragment_type: String,
    content: String,
    authors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PullRequestMetadata {
    Optional,
    Required,
}

/// Strip the trailing `authors: ...` line from fragment content, returning
/// `(description, authors)`. Fails if the field is absent or empty.
fn parse_authors(raw: &str) -> Result<(String, Vec<String>), String> {
    let lines: Vec<&str> = raw.lines().collect();

    let last_idx = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .ok_or("Fragment content is empty")?;

    let last = lines[last_idx].trim();
    let last_lower = last.to_ascii_lowercase();
    let prefix_len = if last_lower.starts_with("authors:") {
        "authors:".len()
    } else if last_lower.starts_with("author:") {
        "author:".len()
    } else {
        return Err(
            "Fragment is missing required 'authors:' field on the last line. \
                    Example: 'authors: github_username'"
                .to_string(),
        );
    };
    let authors_str = &last[prefix_len..];

    let authors: Vec<String> = authors_str
        .split(',')
        .map(|s| s.trim().trim_start_matches('@').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if authors.is_empty() {
        return Err("'authors:' field must list at least one GitHub username".to_string());
    }

    let description = lines[..last_idx].join("\n").trim_end().to_string();
    Ok((description, authors))
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

/// Validate a fragment filename, returning (description, fragment_type).
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

        let (content, authors) =
            parse_authors(raw.trim()).map_err(|e| format!("{} (in {})", e, path.display()))?;

        Ok(Fragment {
            pr_numbers: Vec::new(),
            fragment_type: fragment_type.to_ascii_lowercase(),
            content,
            authors,
        })
    }

    fn collect_fragments(
        &self,
        pull_request_metadata: PullRequestMetadata,
    ) -> Result<BTreeMap<String, Vec<Fragment>>, String> {
        if pull_request_metadata == PullRequestMetadata::Required {
            self.ensure_complete_history()?;
        }

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
                section.push_str(&format!("\n### {type_heading}\n\n"));
                for fragment in fragments {
                    let indented = Self::indent_continuation(&fragment.content);
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
                    section.push_str(&format!("- {indented}\n\n  *{attribution}*\n"));
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

    /// Validate changelog fragment filenames added on the current branch vs origin/main.
    pub fn check_fragments(&self) -> Result<(), String> {
        let output = std::process::Command::new("git")
            .args([
                "diff",
                "--name-only",
                "--diff-filter=A",
                "--merge-base",
                "origin/main",
                "changelog.d",
            ])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| format!("Failed to run git diff: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let filenames: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        if filenames.is_empty() {
            return Err(indoc::indoc! {"
                No changelog fragments detected.
                If no changes necessitate user-facing explanations, add the GH label 'no-changelog'.
                Otherwise, add changelog fragments to changelog.d/
                For details, see 'changelog.d/README.md'"}
            .to_string());
        }

        for path in &filenames {
            let filename = Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(path);

            if filename == "README.md" {
                continue;
            }

            println!("validating '{filename}'");
            validate_fragment_filename(filename)?;
            let full_path = self.repo_root.join(path);
            Self::parse_fragment(&full_path)?;
        }

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
        assert_eq!(desc, "Fixed a bug.");
        assert_eq!(authors, vec!["alice"]);
    }

    #[test]
    fn parse_authors_multiple() {
        let (desc, authors) = parse_authors("A feature.\n\nauthors: alice, bob").unwrap();
        assert_eq!(desc, "A feature.");
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn parse_authors_singular_accepted() {
        let (desc, authors) = parse_authors("Fixed a bug.\n\nauthor: alice").unwrap();
        assert_eq!(desc, "Fixed a bug.");
        assert_eq!(authors, vec!["alice"]);
    }

    #[test]
    fn parse_authors_at_prefix_stripped() {
        let (_, authors) = parse_authors("Fixed a bug.\n\nauthors: @alice, @bob").unwrap();
        assert_eq!(authors, vec!["alice", "bob"]);
    }

    #[test]
    fn parse_authors_capital_key_accepted() {
        let (desc, authors) = parse_authors("Fixed a bug.\n\nAuthors: alice").unwrap();
        assert_eq!(desc, "Fixed a bug.");
        assert_eq!(authors, vec!["alice"]);
    }

    #[test]
    fn parse_authors_missing() {
        let err = parse_authors("Fixed a bug.").unwrap_err();
        assert!(err.contains("missing required 'authors:'"), "{err}");
    }

    #[test]
    fn parse_authors_empty_value() {
        let err = parse_authors("Fixed a bug.\n\nauthors:").unwrap_err();
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
        assert!(err.contains("missing required 'authors:'"), "{err}");
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
        ] {
            std::process::Command::new("git")
                .args(&cmd)
                .current_dir(repo)
                .output()
                .unwrap();
        }

        let changelog_dir = repo.join("changelog.d");
        fs::create_dir(&changelog_dir).unwrap();
        fs::write(changelog_dir.join("README.md"), "# Changelog fragments").unwrap();

        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap();

        for (name, content) in fragments {
            fs::write(changelog_dir.join(name), content).unwrap();
        }

        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(repo)
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "add fragments"])
            .current_dir(repo)
            .output()
            .unwrap();

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
            }],
        );
        grouped.insert(
            "breaking".to_string(),
            vec![Fragment {
                pr_numbers: vec![10],
                fragment_type: "breaking".to_string(),
                content: "Removed old API".to_string(),
                authors: vec!["bob".to_string()],
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
            "breaking".to_string(),
            vec![Fragment {
                pr_numbers: vec![99],
                fragment_type: "breaking".to_string(),
                content: "Removed the old API.\n\nMigrate by changing `foo()` to `bar()`."
                    .to_string(),
                authors: vec!["alice".to_string()],
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(2, 0, 0), "2026-04-17");

        let expected = indoc! {"
            ## [2.0.0 (2026-04-17)](https://github.com/vectordotdev/vrl/releases/tag/v2.0.0)

            ### Breaking Changes & Upgrade Guide

            - Removed the old API.

              Migrate by changing `foo()` to `bar()`.

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
