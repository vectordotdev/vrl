use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Fragment types in display order.
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

    fn changelog_path(&self) -> PathBuf {
        self.repo_root.join("CHANGELOG.md")
    }

    /// Parse a fragment filename like "1234.feature.md" and read its content.
    fn parse_fragment(path: &Path) -> Result<Fragment, String> {
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| format!("Invalid fragment path: {}", path.display()))?;

        let parts: Vec<&str> = filename.splitn(3, '.').collect();
        if parts.len() != 3 || parts[2] != "md" {
            return Err(format!(
                "Invalid fragment filename '{filename}': expected '<pr_number>.<type>.md'"
            ));
        }

        let pr_number = parts[0].to_string();
        if !pr_number.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "Invalid fragment filename '{filename}': first segment must be a PR number"
            ));
        }

        let fragment_type = parts[1].to_string();
        if !FRAGMENT_TYPES.iter().any(|(t, _)| *t == fragment_type) {
            let valid: Vec<&str> = FRAGMENT_TYPES.iter().map(|(t, _)| *t).collect();
            return Err(format!(
                "Invalid fragment type '{fragment_type}' in '{filename}'. Valid types: {}",
                valid.join(", ")
            ));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?
            .trim()
            .to_string();

        Ok(Fragment {
            pr_number,
            fragment_type,
            content,
        })
    }

    /// Collect all fragments from changelog.d/, grouped by type.
    fn collect_fragments(&self) -> Result<BTreeMap<String, Vec<Fragment>>, String> {
        let changelog_dir = self.changelog_dir();
        let mut grouped: BTreeMap<String, Vec<Fragment>> = BTreeMap::new();

        let entries = std::fs::read_dir(&changelog_dir)
            .map_err(|e| format!("Failed to read changelog.d/: {e}"))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if filename == "README.md" {
                continue;
            }

            let fragment = Self::parse_fragment(&path)?;
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

    /// Render a changelog section from grouped fragments for a given version and date.
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
                    let pr_url = format!(
                        "https://github.com/vectordotdev/vrl/pull/{}",
                        fragment.pr_number
                    );
                    section.push_str(&format!("- {}\n\n  ({})\n", fragment.content, pr_url));
                }
            }
        }

        section
    }

    /// Generate the markdown section for a release (without writing it).
    pub fn generate_section(&self, version: &semver::Version) -> Result<String, String> {
        let grouped = self.collect_fragments()?;
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        Ok(Self::render_section(&grouped, version, &date))
    }

    /// Generate the changelog section and insert it into CHANGELOG.md,
    /// then remove the fragment files.
    pub fn generate_and_apply(&self, version: &semver::Version) -> Result<(), String> {
        let section = self.generate_section(version)?;

        // Insert into CHANGELOG.md after the marker
        let changelog_path = self.changelog_path();
        let content = std::fs::read_to_string(&changelog_path)
            .map_err(|e| format!("Failed to read CHANGELOG.md: {e}"))?;

        let new_content = Self::insert_section(&content, &section)?;

        std::fs::write(&changelog_path, new_content)
            .map_err(|e| format!("Failed to write CHANGELOG.md: {e}"))?;

        println!("Updated CHANGELOG.md with {version} section.");

        self.remove_fragments()?;

        println!("Removed changelog fragments.");
        Ok(())
    }

    /// Insert a rendered section into changelog content after the marker.
    fn insert_section(content: &str, section: &str) -> Result<String, String> {
        let marker_pos = content
            .find(CHANGELOG_MARKER)
            .ok_or_else(|| format!("Could not find marker '{CHANGELOG_MARKER}' in CHANGELOG.md"))?;

        let insert_pos = marker_pos + CHANGELOG_MARKER.len();
        let mut new_content = String::with_capacity(content.len() + section.len() + 2);
        new_content.push_str(&content[..insert_pos]);
        new_content.push('\n');
        new_content.push_str(section);
        new_content.push('\n');
        new_content.push_str(&content[insert_pos..]);

        Ok(new_content)
    }

    /// Remove all fragment files (except README.md) from changelog.d/.
    fn remove_fragments(&self) -> Result<(), String> {
        let changelog_dir = self.changelog_dir();
        let entries = std::fs::read_dir(&changelog_dir)
            .map_err(|e| format!("Failed to read changelog.d/: {e}"))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            if filename == "README.md" {
                continue;
            }
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
        }

        Ok(())
    }

    /// Validate changelog fragment filenames (replaces check_changelog_fragments.sh).
    /// Checks fragments added on the current branch vs origin/main.
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
        let fragments: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();

        if fragments.is_empty() {
            return Err("No changelog fragments detected.\n\
                 If no changes necessitate user-facing explanations, add the GH label 'no-changelog'.\n\
                 Otherwise, add changelog fragments to changelog.d/\n\
                 For details, see 'changelog.d/README.md'"
                .to_string());
        }

        let valid_types: Vec<&str> = FRAGMENT_TYPES.iter().map(|(t, _)| *t).collect();

        for fragment_path in &fragments {
            let filename = Path::new(fragment_path)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(fragment_path);

            if filename == "README.md" {
                continue;
            }

            println!("validating '{filename}'");

            let parts: Vec<&str> = filename.splitn(3, '.').collect();
            if parts.len() != 3 {
                return Err(format!(
                    "Invalid fragment filename: wrong number of period delimiters. \
                     Expected '<pr_number>.<fragment_type>.md'. ({filename})"
                ));
            }

            if !parts[0].chars().all(|c| c.is_ascii_digit()) {
                return Err(format!(
                    "Invalid fragment filename: first segment must be PR number. ({filename})"
                ));
            }

            if !valid_types.contains(&parts[1]) {
                return Err(format!(
                    "Invalid fragment filename: fragment type must be one of: {}. ({filename})",
                    valid_types.join(", ")
                ));
            }

            if parts[2] != "md" {
                return Err(format!(
                    "Invalid fragment filename: extension must be markdown (.md). ({filename})"
                ));
            }
        }

        println!("changelog additions are valid.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a temp directory with changelog.d/ and CHANGELOG.md, return its path.
    fn setup_test_repo(fragments: &[(&str, &str)], changelog_content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let changelog_dir = dir.path().join("changelog.d");
        fs::create_dir(&changelog_dir).unwrap();
        fs::write(changelog_dir.join("README.md"), "# Changelog fragments").unwrap();

        for (name, content) in fragments {
            fs::write(changelog_dir.join(name), content).unwrap();
        }

        fs::write(dir.path().join("CHANGELOG.md"), changelog_content).unwrap();

        dir
    }

    const BASIC_CHANGELOG: &str = "\
# Changelog

Changelog is generated from fragments in `changelog.d/` by the `release` crate.

<!-- changelog start -->

## [0.31.0 (2026-03-05)]

### Fixes

- Some old fix.

  (https://github.com/vectordotdev/vrl/pull/100)
";

    #[test]
    fn parse_valid_fragment() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("1234.feature.md");
        fs::write(&path, "Added a cool new thing.").unwrap();

        let fragment = Changelog::parse_fragment(&path).unwrap();
        assert_eq!(fragment.pr_number, "1234");
        assert_eq!(fragment.fragment_type, "feature");
        assert_eq!(fragment.content, "Added a cool new thing.");
    }

    #[test]
    fn parse_fragment_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("42.fix.md");
        fs::write(&path, "\n  Fixed a bug.  \n\n").unwrap();

        let fragment = Changelog::parse_fragment(&path).unwrap();
        assert_eq!(fragment.content, "Fixed a bug.");
    }

    #[test]
    fn parse_fragment_invalid_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("1.unknown.md");
        fs::write(&path, "content").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("Invalid fragment type 'unknown'"), "{err}");
    }

    #[test]
    fn parse_fragment_non_numeric_pr() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc.feature.md");
        fs::write(&path, "content").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("must be a PR number"), "{err}");
    }

    #[test]
    fn parse_fragment_wrong_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("1.feature.txt");
        fs::write(&path, "content").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("expected '<pr_number>.<type>.md'"), "{err}");
    }

    #[test]
    fn parse_fragment_too_few_dots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("1.md");
        fs::write(&path, "content").unwrap();

        let err = Changelog::parse_fragment(&path).unwrap_err();
        assert!(err.contains("expected '<pr_number>.<type>.md'"), "{err}");
    }

    #[test]
    fn collect_fragments_groups_by_type() {
        let dir = setup_test_repo(
            &[
                ("10.feature.md", "New feature A"),
                ("11.feature.md", "New feature B"),
                ("20.fix.md", "Fixed bug"),
            ],
            BASIC_CHANGELOG,
        );

        let changelog = Changelog::new(dir.path());
        let grouped = changelog.collect_fragments().unwrap();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["feature"].len(), 2);
        assert_eq!(grouped["fix"].len(), 1);
    }

    #[test]
    fn collect_fragments_skips_readme() {
        let dir = setup_test_repo(&[("10.feature.md", "A feature")], BASIC_CHANGELOG);

        let changelog = Changelog::new(dir.path());
        let grouped = changelog.collect_fragments().unwrap();

        assert_eq!(grouped.len(), 1);
        assert!(!grouped.contains_key("README"));
    }

    #[test]
    fn collect_fragments_errors_when_empty() {
        let dir = setup_test_repo(&[], BASIC_CHANGELOG);

        let changelog = Changelog::new(dir.path());
        let err = changelog.collect_fragments().unwrap_err();
        assert!(err.contains("No changelog fragments found"), "{err}");
    }

    #[test]
    fn render_section_orders_by_type() {
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

        let version = semver::Version::new(1, 0, 0);
        let section = Changelog::render_section(&grouped, &version, "2026-04-16");

        // Breaking should come before fixes (matches FRAGMENT_TYPES order)
        let breaking_pos = section.find("Breaking Changes").unwrap();
        let fix_pos = section.find("Fixes").unwrap();
        assert!(breaking_pos < fix_pos);
    }

    #[test]
    fn render_section_format() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "feature".to_string(),
            vec![Fragment {
                pr_number: "42".to_string(),
                fragment_type: "feature".to_string(),
                content: "Added something cool".to_string(),
            }],
        );

        let version = semver::Version::new(1, 2, 0);
        let section = Changelog::render_section(&grouped, &version, "2026-04-16");

        assert!(section.starts_with("## [1.2.0 (2026-04-16)]\n"));
        assert!(section.contains("### New Features\n"));
        assert!(section.contains("- Added something cool\n"));
        assert!(section.contains("(https://github.com/vectordotdev/vrl/pull/42)"));
    }

    #[test]
    fn insert_section_places_after_marker() {
        let section = "## [1.0.0 (2026-04-16)]\n\n### Fixes\n\n- Fixed.\n";
        let result = Changelog::insert_section(BASIC_CHANGELOG, section).unwrap();

        // New section should appear between the marker and the old 0.31.0 section
        let new_pos = result.find("## [1.0.0").unwrap();
        let old_pos = result.find("## [0.31.0").unwrap();
        assert!(new_pos < old_pos);

        // Marker should still be present
        assert!(result.contains(CHANGELOG_MARKER));
    }

    #[test]
    fn insert_section_errors_without_marker() {
        let no_marker = "# Changelog\n\nSome content.\n";
        let err = Changelog::insert_section(no_marker, "## [1.0.0]\n").unwrap_err();
        assert!(err.contains("Could not find marker"), "{err}");
    }

    #[test]
    fn generate_and_apply_updates_changelog_and_removes_fragments() {
        let dir = setup_test_repo(
            &[("10.feature.md", "New feature"), ("20.fix.md", "Bug fix")],
            BASIC_CHANGELOG,
        );

        let changelog = Changelog::new(dir.path());
        let version = semver::Version::new(1, 0, 0);
        changelog.generate_and_apply(&version).unwrap();

        // CHANGELOG.md should have the new section
        let content = fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap();
        assert!(content.contains("## [1.0.0"));
        assert!(content.contains("New feature"));
        assert!(content.contains("Bug fix"));

        // Fragments should be removed, README.md should remain
        let remaining: Vec<_> = fs::read_dir(dir.path().join("changelog.d"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(remaining, vec!["README.md"]);
    }

    #[test]
    fn render_section_skips_missing_types() {
        let mut grouped = BTreeMap::new();
        grouped.insert(
            "fix".to_string(),
            vec![Fragment {
                pr_number: "1".to_string(),
                fragment_type: "fix".to_string(),
                content: "A fix".to_string(),
            }],
        );

        let version = semver::Version::new(0, 1, 0);
        let section = Changelog::render_section(&grouped, &version, "2026-01-01");

        assert!(section.contains("### Fixes"));
        assert!(!section.contains("### New Features"));
        assert!(!section.contains("### Breaking"));
        assert!(!section.contains("### Security"));
    }
}
