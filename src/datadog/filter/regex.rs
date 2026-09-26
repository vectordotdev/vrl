use regex::Regex;

/// Returns compiled word boundary regex.
///
/// The `(?s)` flag makes `.` match any character including `\n`, so that
/// wildcards (`*`) match across newlines in multi-line values.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[allow(clippy::module_name_repetitions)] // Renaming is a breaking change.
#[must_use]
pub fn word_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        r"(?s)\b{}\b",
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}

/// Returns compiled wildcard regex.
///
/// The `(?s)` flag makes `.` match any character including `\n`, so that
/// wildcards (`*`) match values with trailing or embedded newlines.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[allow(clippy::module_name_repetitions)] // Renaming is a breaking change.
#[must_use]
pub fn wildcard_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        "(?s)^{}$",
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_matches_trailing_newline() {
        assert!(wildcard_regex("*").is_match("hello world\n"));
        assert!(wildcard_regex("*some*").is_match("here is some content\n"));
        assert!(wildcard_regex("hello*").is_match("hello world\n"));
    }

    #[test]
    fn wildcard_matches_embedded_newline() {
        assert!(wildcard_regex("*").is_match("line1\nline2"));
        assert!(wildcard_regex("*line2").is_match("line1\nline2"));
        assert!(wildcard_regex("a*b").is_match("a\nb"));
    }

    #[test]
    fn wildcard_does_not_over_match() {
        // No wildcard at the end: the value must end with the literal.
        assert!(!wildcard_regex("*some").is_match("here is some\n"));
        // Exact patterns still require exact matches.
        assert!(wildcard_regex("abc").is_match("abc"));
        assert!(!wildcard_regex("abc").is_match("abc\n"));
    }

    #[test]
    fn word_matches_across_newline() {
        assert!(word_regex("foo*bar").is_match("foo\nbar"));
        assert!(word_regex("*some*").is_match("has some stuff\n"));
        assert!(!word_regex("foo*bar").is_match("foo\nbaz"));
    }
}
