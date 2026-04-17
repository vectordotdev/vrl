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
    if !valid_types.contains(&parts[1]) {
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

        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?
            .trim()
            .to_string();

        Ok(Fragment {
            pr_number: pr_number.to_string(),
            fragment_type: fragment_type.to_string(),
            content,
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
        let mut section = format!("## [{version} ({date})]\n");

        for (type_key, type_heading) in FRAGMENT_TYPES {
            if let Some(fragments) = grouped.get(*type_key) {
                section.push_str(&format!("\n### {type_heading}\n\n"));
                for fragment in fragments {
                    let indented = Self::indent_continuation(&fragment.content);
                    section.push_str(&format!(
                        "- {indented}\n\n  (https://github.com/vectordotdev/vrl/pull/{})\n",
                        fragment.pr_number
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

    // --- parse_fragment ---

    #[test]
    fn parse_reads_and_trims_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("42.fix.md");
        fs::write(&path, "\n  Fixed a bug.  \n\n").unwrap();

        let fragment = Changelog::parse_fragment(&path).unwrap();
        assert_eq!(fragment.pr_number, "42");
        assert_eq!(fragment.fragment_type, "fix");
        assert_eq!(fragment.content, "Fixed a bug.");
    }

    // --- collect_fragments ---

    #[test]
    fn groups_by_type() {
        let dir = setup_test_repo(&[
            ("10.feature.md", "Feature A"),
            ("11.feature.md", "Feature B"),
            ("20.fix.md", "Bug fix"),
        ]);
        let grouped = Changelog::new(dir.path()).collect_fragments().unwrap();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["feature"].len(), 2);
        assert_eq!(grouped["fix"].len(), 1);
    }

    #[test]
    fn skips_readme() {
        let dir = setup_test_repo(&[("10.feature.md", "A feature")]);
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
            }],
        );
        grouped.insert(
            "breaking".to_string(),
            vec![Fragment {
                pr_number: "10".to_string(),
                fragment_type: "breaking".to_string(),
                content: "Removed old API".to_string(),
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
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(1, 2, 0), "2026-04-16");

        let expected = indoc! {"
            ## [1.2.0 (2026-04-16)]

            ### New Features

            - Added something cool

              (https://github.com/vectordotdev/vrl/pull/42)
        "};
        assert_eq!(section, expected);
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
            }],
        );

        let section =
            Changelog::render_section(&grouped, &semver::Version::new(2, 0, 0), "2026-04-17");

        let expected = indoc! {"
            ## [2.0.0 (2026-04-17)]

            ### Breaking Changes & Upgrade Guide

            - Removed the old API.

              Migrate by changing `foo()` to `bar()`.

              (https://github.com/vectordotdev/vrl/pull/99)
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
        let dir = setup_test_repo(&[("10.feature.md", "New feature"), ("20.fix.md", "Bug fix")]);

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
