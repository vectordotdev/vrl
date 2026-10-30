//! The same breaking-fragment schema used by Vector's changelog tooling.

use std::fmt::Write as _;

#[derive(Debug)]
pub(super) struct BreakingSections {
    title: String,
    pub(super) anchor: String,
    summary: String,
    migration: String,
}

pub(super) fn parse_authors(raw: &str) -> Result<(String, Vec<String>), String> {
    let (body, last) = raw
        .strip_suffix('\n')
        .unwrap_or(raw)
        .rsplit_once('\n')
        .ok_or("last line must be 'authors: <name> [<name> ...]' (no trailing blank lines)")?;
    let names = last
        .strip_prefix("authors: ")
        .ok_or("last line must be 'authors: <name> [<name> ...]' (no trailing blank lines)")?;
    if names.trim().is_empty() {
        return Err("authors line must list at least one GitHub username".into());
    }
    if names.contains('@') || names.contains(',') {
        return Err("authors must be space-delimited GitHub usernames without '@'".into());
    }
    let authors: Vec<String> = names.split_whitespace().map(str::to_string).collect();
    if authors.iter().any(|name| name == "TODO_your_gh_handle") {
        return Err("replace the authors placeholder with a real GitHub username".into());
    }
    Ok((body.to_string(), authors))
}

impl BreakingSections {
    pub(super) fn parse(body: &str) -> Result<Self, String> {
        const SUMMARY: &str = "\n## Summary\n";
        const MIGRATION: &str = "\n## Migration\n";

        // Match Vector: normalize Windows line endings and allow an empty final section.
        let mut normalized = body.replace("\r\n", "\n");
        if !normalized.ends_with('\n') {
            normalized.push('\n');
        }
        let title_body = normalized
            .strip_prefix("# ")
            .ok_or("first line must be an H1 title (`# ...`)")?;
        let (title_line, after_title) = title_body.split_once('\n').unwrap_or((title_body, ""));
        let (title, anchor) = split_title_and_anchor(title_line);
        if title.is_empty() || title.contains("TODO") {
            return Err("H1 title must be nonempty and contain no TODO placeholder".into());
        }
        if body.lines().any(|line| line.starts_with("TODO ")) {
            return Err("replace TODO placeholder lines with real content".into());
        }
        if after_title.matches(SUMMARY).count() != 1 {
            return Err("exactly one `## Summary` section is required".into());
        }
        if after_title.matches(MIGRATION).count() != 1 {
            return Err("exactly one `## Migration` section is required".into());
        }
        let summary_pos = after_title.find(SUMMARY).expect("checked above");
        let migration_pos = after_title.find(MIGRATION).expect("checked above");
        if summary_pos >= migration_pos {
            return Err("`## Summary` must come before `## Migration`".into());
        }
        if !after_title[..summary_pos].trim().is_empty() {
            return Err("move content between the title and `## Summary` into a section".into());
        }
        let summary = after_title[summary_pos + SUMMARY.len()..migration_pos].trim();
        if summary.is_empty() {
            return Err("`## Summary` must not be empty".into());
        }
        let anchor = anchor.map_or_else(|| slugify(title), str::to_string);
        if !is_valid_anchor(&anchor) {
            return Err(format!(
                "invalid upgrade-guide anchor '{anchor}': use a unique lowercase kebab-case slug (override with `{{#some-slug}}`)"
            ));
        }
        Ok(Self {
            title: title.to_string(),
            anchor,
            summary: summary.to_string(),
            migration: after_title[migration_pos + MIGRATION.len()..]
                .trim()
                .to_string(),
        })
    }

    pub(super) fn render(&self, version: &semver::Version) -> String {
        // GitHub does not understand Hugo's {#anchor} syntax. Emit an explicit anchor,
        // with headings nested under the release (H2) and change category (H3).
        let mut rendered = format!(
            "<a id=\"v{version}-{}\"></a>\n\n#### {}\n\n##### Summary\n\n{}\n\n##### Migration\n",
            self.anchor,
            self.title,
            nest_headings(&self.summary)
        );
        if !self.migration.is_empty() {
            write!(rendered, "\n{}\n", nest_headings(&self.migration))
                .expect("writing to a String cannot fail");
        }
        rendered.trim_end().to_string()
    }
}

fn split_title_and_anchor(line: &str) -> (&str, Option<&str>) {
    let trimmed = line.trim_end();
    if let Some(prefix) = trimmed.strip_suffix('}')
        && let Some(open) = prefix.rfind("{#")
    {
        return (prefix[..open].trim(), Some(&prefix[open + 2..]));
    }
    (trimmed, None)
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn is_valid_anchor(anchor: &str) -> bool {
    // Keep Vector's reserved anchors so these fragments can be inlined in its release page.
    !anchor.is_empty()
        && ![
            "vector-breaking-changes",
            "vector-upgrade-guide",
            "upgrade-guide",
        ]
        .contains(&anchor)
        && !anchor.starts_with('-')
        && !anchor.ends_with('-')
        && anchor
            .bytes()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == b'-')
}

/// Demote authored headings alongside Summary/Migration without altering fenced examples.
fn nest_headings(markdown: &str) -> String {
    let mut fence: Option<(u8, usize)> = None;
    markdown
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            let indent = line.len() - trimmed.len();
            let first = trimmed.as_bytes().first().copied().unwrap_or_default();
            if indent <= 3 && (first == b'`' || first == b'~') {
                let count = trimmed.bytes().take_while(|ch| *ch == first).count();
                if let Some((delimiter, length)) = fence {
                    if first == delimiter && count >= length && trimmed[count..].trim().is_empty() {
                        fence = None;
                    }
                } else if count >= 3 {
                    fence = Some((first, count));
                }
            } else if fence.is_none() && indent <= 3 {
                let count = trimmed.bytes().take_while(|ch| *ch == b'#').count();
                if (1..=6).contains(&count)
                    && (trimmed.len() == count || trimmed.as_bytes()[count].is_ascii_whitespace())
                {
                    return format!(
                        "{}{}{}",
                        &line[..indent],
                        "#".repeat((count + 3).min(6)),
                        &trimmed[count..]
                    );
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
