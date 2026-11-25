use regex::Regex;

/// Returns compiled word boundary regex.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[allow(clippy::module_name_repetitions)] // Renaming is a breaking change.
#[must_use]
pub fn word_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        r"\b{}\b",
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}

/// Returns compiled wildcard regex.
///
/// # Panics
/// Panics if an invalid wildcard regex is provided.
#[allow(clippy::module_name_repetitions)] // Renaming is a breaking change.
#[must_use]
pub fn wildcard_regex(to_match: &str) -> Regex {
    Regex::new(&format!(
        "^{}$",
        regex::escape(to_match).replace("\\*", ".*")
    ))
    .expect("invalid wildcard regex")
}

/// Matches values the same way as [`wildcard_regex`], with plain string comparisons when `*`
/// only appears at the ends of the pattern.
#[derive(Clone, Debug)]
pub struct WildcardMatcher(Strategy);

#[derive(Clone, Debug)]
enum Strategy {
    Exact(String),
    Prefix(String),
    Suffix(String),
    Contains(String),
    Regex(Regex),
}

impl WildcardMatcher {
    /// # Panics
    /// Panics if an invalid wildcard regex is provided.
    #[must_use]
    pub fn new(to_match: &str) -> Self {
        let without_leading = to_match.trim_start_matches('*');
        let literal = without_leading.trim_end_matches('*');
        // The `\n` checks in `is_match` assume the literal holds none.
        if literal.contains(['*', '\n']) {
            return Self(Strategy::Regex(wildcard_regex(to_match)));
        }

        let leading = without_leading.len() < to_match.len();
        let trailing = literal.len() < without_leading.len();
        let literal = literal.to_owned();
        Self(match (leading, trailing) {
            (false, false) => Strategy::Exact(literal),
            (false, true) => Strategy::Prefix(literal),
            (true, false) => Strategy::Suffix(literal),
            (true, true) => Strategy::Contains(literal),
        })
    }

    #[must_use]
    pub fn is_match(&self, value: &str) -> bool {
        // Like `.*` in the regex, a `*` never spans a `\n`, and the literal holds none, so any
        // `\n` in the value rules out a match.
        match &self.0 {
            Strategy::Exact(literal) => value == literal,
            Strategy::Prefix(literal) => {
                value.starts_with(literal.as_str()) && !value.contains('\n')
            }
            Strategy::Suffix(literal) => value.ends_with(literal.as_str()) && !value.contains('\n'),
            Strategy::Contains(literal) => {
                value.contains(literal.as_str()) && !value.contains('\n')
            }
            Strategy::Regex(re) => re.is_match(value),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    proptest::proptest! {
        #[test]
        fn wildcard_matcher_agrees_with_wildcard_regex(
            pattern in "[ab*?\n]{0,6}",
            value in "[ab?\n]{0,8}",
        ) {
            proptest::prop_assert_eq!(
                WildcardMatcher::new(&pattern).is_match(&value),
                wildcard_regex(&pattern).is_match(&value),
            );
        }
    }
}
