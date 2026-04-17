use std::path::Path;
use toml_edit::DocumentMut;

/// Read the current version from the root Cargo.toml.
pub fn read_version(repo_root: &Path) -> Result<semver::Version, String> {
    let path = repo_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let doc: DocumentMut = content
        .parse()
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;

    let version_str = doc["package"]["version"]
        .as_str()
        .ok_or("Missing [package].version in Cargo.toml")?;

    semver::Version::parse(version_str)
        .map_err(|e| format!("Invalid version '{version_str}' in Cargo.toml: {e}"))
}

/// Write a new version into the root Cargo.toml, preserving formatting.
pub fn write_version(repo_root: &Path, version: &semver::Version) -> Result<(), String> {
    let path = repo_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;

    doc["package"]["version"] = toml_edit::value(version.to_string());

    std::fs::write(&path, doc.to_string())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;

    Ok(())
}

/// Resolve a version argument into a concrete semver::Version.
///
/// - `None` → bump minor
/// - `Some("major")` / `Some("minor")` / `Some("patch")` → bump accordingly
/// - `Some("1.2.3")` → parse as exact version
pub fn resolve(arg: Option<&str>, current: &semver::Version) -> Result<semver::Version, String> {
    match arg {
        None | Some("minor") => {
            let mut v = current.clone();
            v.minor += 1;
            v.patch = 0;
            v.pre = semver::Prerelease::EMPTY;
            Ok(v)
        }
        Some("major") => {
            let mut v = current.clone();
            v.major += 1;
            v.minor = 0;
            v.patch = 0;
            v.pre = semver::Prerelease::EMPTY;
            Ok(v)
        }
        Some("patch") => {
            let mut v = current.clone();
            v.patch += 1;
            v.pre = semver::Prerelease::EMPTY;
            Ok(v)
        }
        Some(exact) => {
            semver::Version::parse(exact).map_err(|e| format!("Invalid version '{exact}': {e}"))
        }
    }
}
