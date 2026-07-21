pub mod query_string;
pub mod ruby_hash;
pub mod xml;

// nom's convert_error formats a caret at column `n` using `{caret:>n$}`, which
// panics in Rust ≥ 1.87 when n ≥ 65536 (fmt width is capped at 0xffff).
// Until nom#1868 lands, we guard against that here.
pub(crate) fn safe_convert_error(
    input: &str,
    e: nom_language::error::VerboseError<&str>,
) -> String {
    use nom::Offset;

    // For each error entry, check whether its column would overflow the limit.
    let overflows = e.errors.iter().any(|(substring, _)| {
        let offset = input.offset(substring);
        let line_begin = input.as_bytes()[..offset]
            .iter()
            .rev()
            .position(|&b| b == b'\n')
            .map(|pos| offset - pos)
            .unwrap_or(0);
        let column = input[line_begin..].offset(substring) + 1;
        column >= 65535
    });

    if overflows {
        // Compute position of the first error for a useful message.
        let (substring, _) = &e.errors[0];
        let offset = input.offset(substring);
        let prefix = &input.as_bytes()[..offset];
        let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        let line_begin = prefix
            .iter()
            .rev()
            .position(|&b| b == b'\n')
            .map(|pos| offset - pos)
            .unwrap_or(0);
        let column = input[line_begin..].offset(substring) + 1;
        format!("parse error at line {line}, column {column} (line too long to display context)")
    } else {
        #[allow(clippy::disallowed_methods)]
        nom_language::error::convert_error(input, e)
    }
}
