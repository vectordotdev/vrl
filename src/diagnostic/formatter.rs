use std::fmt;

use super::DiagnosticList;

/// A formatter to display diagnostics tied to a given source.
pub struct Formatter<'a> {
    source: &'a str,
    diagnostics: DiagnosticList,
    color: bool,
}

impl<'a> Formatter<'a> {
    pub fn new(source: &'a str, diagnostics: impl Into<DiagnosticList>) -> Self {
        Self {
            source,
            diagnostics: diagnostics.into(),
            color: false,
        }
    }

    #[must_use]
    pub fn colored(mut self) -> Self {
        self.color = true;
        self
    }

    pub fn enable_colors(&mut self, color: bool) {
        self.color = color
    }

    #[must_use]
    pub fn diagnostics(&self) -> &DiagnosticList {
        &self.diagnostics
    }
}

impl<'a> fmt::Display for Formatter<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::str::from_utf8;

        use codespan_reporting::{files::SimpleFile, term};
        use termcolor::Buffer;

        if self.diagnostics.is_empty() {
            return Ok(());
        }

        let file = SimpleFile::new("", self.source);
        let config = term::Config::default();
        let mut buffer = if self.color {
            Buffer::ansi()
        } else {
            Buffer::no_color()
        };

        f.write_str("\n")?;

        for diagnostic in self.diagnostics.iter() {
            term::emit(&mut buffer, &config, &file, &diagnostic.clone().into())
                .map_err(|_| fmt::Error)?;
        }

        // Diagnostic messages can contain whitespace at the end of some lines.
        // This causes problems when used in our UI testing, as editors often
        // strip end-of-line whitespace. Removing this has no actual visual
        // impact.
        let string = from_utf8(buffer.as_slice())
            .map_err(|_| fmt::Error)?
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");

        f.write_str(&string)
    }
}
