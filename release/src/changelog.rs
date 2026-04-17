use std::collections::BTreeMap;
use std::path::Path;

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

struct Fragment {
    pr_number: String,
    fragment_type: String,
    content: String,
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
fn collect_fragments(repo_root: &Path) -> Result<BTreeMap<String, Vec<Fragment>>, String> {
    let changelog_dir = repo_root.join("changelog.d");
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

        let fragment = parse_fragment(&path)?;
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

/// Generate the markdown section for a release (without writing it).
pub fn generate_section(repo_root: &Path, version: &semver::Version) -> Result<String, String> {
    let grouped = collect_fragments(repo_root)?;
    let date = chrono::Utc::now().format("%Y-%m-%d");

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

    Ok(section)
}

/// Generate the changelog section and insert it into CHANGELOG.md,
/// then remove the fragment files.
pub fn generate_and_apply(repo_root: &Path, version: &semver::Version) -> Result<(), String> {
    let section = generate_section(repo_root, version)?;

    // Insert into CHANGELOG.md after the marker
    let changelog_path = repo_root.join("CHANGELOG.md");
    let content = std::fs::read_to_string(&changelog_path)
        .map_err(|e| format!("Failed to read CHANGELOG.md: {e}"))?;

    let Some(marker_pos) = content.find(CHANGELOG_MARKER) else {
        return Err(format!(
            "Could not find marker '{CHANGELOG_MARKER}' in CHANGELOG.md"
        ));
    };

    let insert_pos = marker_pos + CHANGELOG_MARKER.len();
    let mut new_content = String::with_capacity(content.len() + section.len() + 1);
    new_content.push_str(&content[..insert_pos]);
    new_content.push('\n');
    new_content.push_str(&section);
    new_content.push('\n');
    new_content.push_str(&content[insert_pos..]);

    std::fs::write(&changelog_path, new_content)
        .map_err(|e| format!("Failed to write CHANGELOG.md: {e}"))?;

    println!("Updated CHANGELOG.md with {version} section.");

    // Remove fragment files
    let changelog_dir = repo_root.join("changelog.d");
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

    println!("Removed changelog fragments.");
    Ok(())
}

/// Validate changelog fragment filenames (replaces check_changelog_fragments.sh).
/// Checks fragments added on the current branch vs origin/main.
pub fn check_fragments(repo_root: &Path) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args([
            "diff",
            "--name-only",
            "--diff-filter=A",
            "--merge-base",
            "origin/main",
            "changelog.d",
        ])
        .current_dir(repo_root)
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
