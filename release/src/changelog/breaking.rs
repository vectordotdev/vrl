//! The same breaking-fragment schema used by Vector's changelog tooling.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

#[derive(Debug)]
pub(super) struct BreakingSections {
    title: String,
    pub(super) anchor: String,
    summary: String,
    migration: String,
}

impl BreakingSections {
    pub(super) fn parse(body: &str) -> Result<Self, String> {
        const SUMMARY: &str = "## Summary";
        const MIGRATION: &str = "## Migration";

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
        let mut summaries = Vec::new();
        let mut migrations = Vec::new();
        for (event, range) in Parser::new(after_title).into_offset_iter() {
            if !matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            ) {
                continue;
            }
            // Keep the existing literal section syntax, including the leading newline.
            if range.start == 0 || after_title.as_bytes()[range.start - 1] != b'\n' {
                continue;
            }
            match after_title[range.clone()].trim_end_matches('\n') {
                SUMMARY => summaries.push(range),
                MIGRATION => migrations.push(range),
                _ => {}
            }
        }
        let [summary_header] = summaries.as_slice() else {
            return Err("exactly one `## Summary` section is required".into());
        };
        let [migration_header] = migrations.as_slice() else {
            return Err("exactly one `## Migration` section is required".into());
        };
        if summary_header.start >= migration_header.start {
            return Err("`## Summary` must come before `## Migration`".into());
        }
        if !after_title[..summary_header.start].trim().is_empty() {
            return Err("move content between the title and `## Summary` into a section".into());
        }
        let summary = after_title[summary_header.end..migration_header.start].trim_matches('\n');
        if summary.trim().is_empty() {
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
            migration: after_title[migration_header.end..]
                .trim_matches('\n')
                .to_string(),
        })
    }

    pub(super) fn render(&self, version: &semver::Version) -> String {
        format!(
            "<a id=\"v{version}-{}\"></a> **{}**\n\n**Summary**\n\n{}\n\n**Migration**\n\n{}",
            self.anchor, self.title, self.summary, self.migration
        )
        .trim_end_matches('\n')
        .to_string()
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
