use regex::Regex;

/// Returns compiled word boundary regex.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[must_use]
pub fn word_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        r#"\b{}\b"#,
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}

/// Returns compiled wildcard regex.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[must_use]
pub fn wildcard_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        "^{}$",
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}
