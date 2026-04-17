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

/// Validate that an exact version is a single-step bump from current.
/// Accepts: major+1 (with minor/patch reset), minor+1 (with patch reset), or patch+1.
fn validate_bump(current: &semver::Version, new: &semver::Version) -> Result<(), String> {
    if new <= current {
        return Err(format!(
            "New version {new} must be greater than current version {current}."
        ));
    }

    let valid_major = semver::Version::new(current.major + 1, 0, 0);
    let valid_minor = semver::Version::new(current.major, current.minor + 1, 0);
    let valid_patch = semver::Version::new(current.major, current.minor, current.patch + 1);

    if *new != valid_major && *new != valid_minor && *new != valid_patch {
        return Err(format!(
            "Version {new} is not a valid bump from {current}. \
             Expected one of: {valid_major} (major), {valid_minor} (minor), {valid_patch} (patch)."
        ));
    }

    Ok(())
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
            let parsed = semver::Version::parse(exact)
                .map_err(|e| format!("Invalid version '{exact}': {e}"))?;
            validate_bump(current, &parsed)?;
            return Ok(parsed);
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
    fn exact_version_major_bump() {
        assert_eq!(resolve(Some("2.0.0"), &v(1, 2, 3)).unwrap(), v(2, 0, 0));
    }

    #[test]
    fn exact_version_minor_bump() {
        assert_eq!(resolve(Some("1.3.0"), &v(1, 2, 3)).unwrap(), v(1, 3, 0));
    }

    #[test]
    fn exact_version_patch_bump() {
        assert_eq!(resolve(Some("1.2.4"), &v(1, 2, 3)).unwrap(), v(1, 2, 4));
    }

    #[test]
    fn exact_version_rejects_skipped_minor() {
        let err = resolve(Some("1.5.0"), &v(1, 2, 3)).unwrap_err();
        assert!(err.contains("not a valid bump"), "{err}");
    }

    #[test]
    fn exact_version_rejects_skipped_major() {
        let err = resolve(Some("5.0.0"), &v(1, 2, 3)).unwrap_err();
        assert!(err.contains("not a valid bump"), "{err}");
    }

    #[test]
    fn exact_version_rejects_downgrade() {
        let err = resolve(Some("1.0.0"), &v(1, 2, 3)).unwrap_err();
        assert!(err.contains("must be greater"), "{err}");
    }

    #[test]
    fn exact_version_rejects_same() {
        let err = resolve(Some("1.2.3"), &v(1, 2, 3)).unwrap_err();
        assert!(err.contains("must be greater"), "{err}");
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
