use std::path::Path;
use toml_edit::DocumentMut;

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

pub fn write_version(repo_root: &Path, version: &semver::Version) -> Result<(), String> {
    let path = repo_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;

    doc["package"]["version"] = toml_edit::value(version.to_string());

    std::fs::write(&path, doc.to_string())
        .map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Resolve a version argument into a concrete semver::Version.
///
/// - `None` / `"minor"` → bump minor (default)
/// - `"major"` / `"patch"` → bump accordingly
/// - `"1.2.3"` → parse as exact version
pub fn resolve(arg: Option<&str>, current: &semver::Version) -> Result<semver::Version, String> {
    let mut v = current.clone();
    v.pre = semver::Prerelease::EMPTY;

    match arg {
        Some("major") => {
            v.major += 1;
            v.minor = 0;
            v.patch = 0;
        }
        None | Some("minor") => {
            v.minor += 1;
            v.patch = 0;
        }
        Some("patch") => v.patch += 1,
        Some(exact) => {
            return semver::Version::parse(exact)
                .map_err(|e| format!("Invalid version '{exact}': {e}"));
        }
    }

    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;

    fn v(major: u64, minor: u64, patch: u64) -> semver::Version {
        semver::Version::new(major, minor, patch)
    }

    #[test]
    fn default_bumps_minor() {
        assert_eq!(resolve(None, &v(1, 2, 3)).unwrap(), v(1, 3, 0));
    }

    #[test]
    fn explicit_minor() {
        assert_eq!(resolve(Some("minor"), &v(1, 2, 3)).unwrap(), v(1, 3, 0));
    }

    #[test]
    fn major_resets_minor_and_patch() {
        assert_eq!(resolve(Some("major"), &v(1, 2, 3)).unwrap(), v(2, 0, 0));
    }

    #[test]
    fn patch_only_bumps_patch() {
        assert_eq!(resolve(Some("patch"), &v(1, 2, 3)).unwrap(), v(1, 2, 4));
    }

    #[test]
    fn exact_version() {
        assert_eq!(resolve(Some("5.0.0"), &v(1, 2, 3)).unwrap(), v(5, 0, 0));
    }

    #[test]
    fn invalid_exact_version() {
        let err = resolve(Some("not.a.version"), &v(1, 0, 0)).unwrap_err();
        assert!(err.contains("Invalid version"), "{err}");
    }

    #[test]
    fn read_and_write_version_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            indoc! {r#"
                [package]
                name = "test"
                version = "1.2.3"
            "#},
        )
        .unwrap();

        assert_eq!(read_version(dir.path()).unwrap(), v(1, 2, 3));

        write_version(dir.path(), &v(2, 0, 0)).unwrap();
        assert_eq!(read_version(dir.path()).unwrap(), v(2, 0, 0));
    }
}
