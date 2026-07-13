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
    pr_number: String,
    fragment_type: String,
    content: String,
    authors: Vec<String>,
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
        .map(|a| format!("[@{a}](https://github.com/{a})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Validate a fragment filename, returning (pr_number, fragment_type).
fn validate_fragment_filename(filename: &str) -> Result<(&str, &str), String> {
    let parts: Vec<&str> = filename.splitn(3, '.').collect();
    if parts.len() != 3 || parts[2] != "md" {
        return Err(format!(
            "Invalid fragment filename '{filename}': expected '<pr_number>.<type>.md'"
        ));
    }

    if parts[0].is_empty() || !parts[0].chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Invalid fragment filename '{filename}': first segment must be a PR number"
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

        let (pr_number, fragment_type) = validate_fragment_filename(filename)?;

        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

        let (content, authors) =
            parse_authors(raw.trim()).map_err(|e| format!("{} (in {})", e, path.display()))?;

        Ok(Fragment {
            pr_number: pr_number.to_string(),
            fragment_type: fragment_type.to_ascii_lowercase(),
            content,
            authors,
        })
    }

    fn collect_fragments(&self) -> Result<BTreeMap<String, Vec<Fragment>>, String> {
        let mut grouped: BTreeMap<String, Vec<Fragment>> = BTreeMap::new();

        for entry in Self::read_fragment_dir(&self.changelog_dir())? {
            let fragment = Self::parse_fragment(&entry)?;
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
                    let pr = &fragment.pr_number;
                    let url = format!("https://github.com/vectordotdev/vrl/pull/{pr}");
                    section.push_str(&format!(
                        "- {indented}\n\n  [PR #{pr}]({url}) by {authors}\n"
                    ));
                }
            }
        }

        section
    }

    pub fn generate_section(&self, version: &semver::Version) -> Result<String, String> {
        let grouped = self.collect_fragments()?;
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        Ok(Self::render_section(&grouped, version, &date))
    }

    /// Generate the changelog section, insert it into CHANGELOG.md, and remove fragments.
    pub fn generate_and_apply(&self, version: &semver::Version) -> Result<(), String> {
        let section = self.generate_section(version)?;

        let changelog_path = self.repo_root.join("CHANGELOG.md");
        let content = std::fs::read_to_string(&changelog_path)
            .map_err(|e| format!("Failed to read CHANGELOG.md: {e}"))?;

        let new_content = Self::insert_section(&content, &section)?;
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

    /// Validate every fragment file currently on disk in `changelog.d/` —
    /// what the release will consume. Unlike [`check_fragments`], this does
    /// not diff against `origin/main`, so it works on a synced release branch
    /// where no fragments are "newly added" but plenty exist to consume.
    pub fn validate_fragments_on_disk(&self) -> Result<(), String> {
        let paths = Self::read_fragment_dir(&self.changelog_dir())?;
        if paths.is_empty() {
            return Err(
                "No changelog fragments found in changelog.d/ — nothing to release.".to_string(),
            );
        }
        for path in &paths {
            Self::parse_fragment(path)?;
        }
        println!("Validated {} changelog fragment(s).", paths.len());
        Ok(())
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

    // --- validate_fragment_filename ---

    #[test]
    fn valid_filename() {
        let (pr, ty) = validate_fragment_filename("1234.feature.md").unwrap();
        assert_eq!(pr, "1234");
        assert_eq!(ty, "feature");
    }

    #[test]
    fn all_fragment_types_accepted() {
        for (ty, _) in FRAGMENT_TYPES {
            validate_fragment_filename(&format!("1.{ty}.md")).unwrap();
        }
    }

    #[test]
    fn empty_pr_number() {
        let err = validate_fragment_filename(".feature.md").unwrap_err();
        assert!(err.contains("must be a PR number"), "{err}");
    }

    #[test]
    fn invalid_type() {
        let err = validate_fragment_filename("1.unknown.md").unwrap_err();
        assert!(err.contains("Invalid fragment type 'unknown'"), "{err}");
    }

    #[test]
    fn fragment_type_case_insensitive() {
        let (_, ty) = validate_fragment_filename("1234.Fix.md").unwrap();
        assert_eq!(ty, "Fix"); // raw value; normalized to lowercase in parse_fragment
    }

    #[test]
    fn non_numeric_pr() {
        let err = validate_fragment_filename("abc.feature.md").unwrap_err();
        assert!(err.contains("must be a PR number"), "{err}");
    }

    #[test]
    fn wrong_extension() {
        let err = validate_fragment_filename("1.feature.txt").unwrap_err();
        assert!(err.contains("expected '<pr_number>.<type>.md'"), "{err}");
    }

    #[test]
    fn too_few_dots() {
        let err = validate_fragment_filename("1.md").unwrap_err();
        assert!(err.contains("expected '<pr_number>.<type>.md'"), "{err}");
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
        let path = dir.path().join("42.fix.md");
        fs::write(&path, "Fixed a bug.\n\nauthors: alice\n").unwrap();

        let fragment = Changelog::parse_fragment(&path).unwrap();
        assert_eq!(fragment.pr_number, "42");
        assert_eq!(fragment.fragment_type, "fix");
        assert_eq!(fragment.content, "Fixed a bug.");
        assert_eq!(fragment.authors, vec!["alice"]);
    }

    #[test]
    fn parse_fragment_missing_authors_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("42.fix.md");
        fs::write(&path, "Fixed a bug.\n").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("missing required 'authors:'"), "{err}");
    }

    // --- collect_fragments ---

    #[test]
    fn groups_by_type() {
        let dir = setup_test_repo(&[
            ("10.feature.md", "Feature A\n\nauthors: alice"),
            ("11.feature.md", "Feature B\n\nauthors: bob"),
            ("20.fix.md", "Bug fix\n\nauthors: carol"),
        ]);
        let grouped = Changelog::new(dir.path()).collect_fragments().unwrap();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["feature"].len(), 2);
        assert_eq!(grouped["fix"].len(), 1);
    }

    #[test]
    fn skips_readme() {
        let dir = setup_test_repo(&[("10.feature.md", "A feature\n\nauthors: alice")]);
        let grouped = Changelog::new(dir.path()).collect_fragments().unwrap();

        assert_eq!(grouped.len(), 1);
        assert!(!grouped.contains_key("README"));
    }

    #[test]
    fn errors_when_empty() {
        let dir = setup_test_repo(&[]);
        let err = Changelog::new(dir.path()).collect_fragments().unwrap_err();
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
        let dir = setup_git_check_repo(&[("123.fix.md", "Fixed something.\n\nauthors: alice")]);
        let result = Changelog::new(dir.path()).check_fragments();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn check_fragments_missing_authors_fails() {
        let dir = setup_git_check_repo(&[("123.fix.md", "Fixed something.\n")]);
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
                pr_number: "20".to_string(),
                fragment_type: "fix".to_string(),
                content: "Fixed a bug".to_string(),
                authors: vec!["alice".to_string()],
            }],
        );
        grouped.insert(
            "breaking".to_string(),
            vec![Fragment {
                pr_number: "10".to_string(),
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
                pr_number: "1".to_string(),
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
                pr_number: "42".to_string(),
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

              [PR #42](https://github.com/vectordotdev/vrl/pull/42) by [@alice](https://github.com/alice)
        "};
        assert_eq!(section, expected);
    }

    #[test]
    fn section_format_multiple_authors() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_number: "7".to_string(),
                fragment_type: "fix".to_string(),
                content: "A fix".to_string(),
                authors: vec!["alice".to_string(), "bob".to_string()],
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(0, 1, 0), "2026-01-01");

        assert!(
            section.contains("[@alice](https://github.com/alice), [@bob](https://github.com/bob)"),
            "{section}"
        );
    }

    #[test]
    fn multiline_fragment_indents_continuation() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "breaking".to_string(),
            vec![Fragment {
                pr_number: "99".to_string(),
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

              [PR #99](https://github.com/vectordotdev/vrl/pull/99) by [@alice](https://github.com/alice)
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

    // --- generate_and_apply (integration) ---

    #[test]
    fn updates_changelog_and_removes_fragments() {
        let dir = setup_test_repo(&[
            ("10.feature.md", "New feature\n\nauthors: alice"),
            ("20.fix.md", "Bug fix\n\nauthors: bob"),
        ]);

        Changelog::new(dir.path())
            .generate_and_apply(&semver::Version::new(1, 0, 0))
            .unwrap();

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
