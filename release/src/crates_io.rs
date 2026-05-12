use serde::Deserialize;

#[derive(Deserialize)]
struct CrateResponse {
    versions: Vec<CrateVersion>,
}

#[derive(Deserialize)]
struct CrateVersion {
    num: String,
}

/// Fetch published versions from crates.io and fail if the given version already exists.
pub fn assert_not_published(version: &semver::Version) -> Result<(), String> {
    let version_str = version.to_string();
    println!("Checking crates.io for existing version {version_str}...");

    let body: CrateResponse = ureq::get("https://crates.io/api/v1/crates/vrl")
        .header("User-Agent", "vrl-release-tool")
        .call()
        .map_err(|e| format!("Failed to query crates.io: {e}"))?
        .body_mut()
        .read_json()
        .map_err(|e| format!("Failed to parse crates.io response: {e}"))?;

    if body.versions.iter().any(|v| v.num == version_str) {
        return Err(format!(
            "Version {version_str} is already published on crates.io."
        ));
    }

    println!("Version {version_str} is not yet published. Good.");
    Ok(())
}
