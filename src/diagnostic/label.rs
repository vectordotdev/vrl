use codespan_reporting::diagnostic;

use super::Span;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Label {
    pub message: String,
    pub primary: bool,
    pub span: Span,
}

impl Label {
    pub fn primary(message: impl Into<String>, span: impl Into<Span>) -> Self {
        Self {
            message: message.into(),
            primary: true,
            span: span.into(),
        }
    }

    pub fn context(message: impl Into<String>, span: impl Into<Span>) -> Self {
        Self {
            message: message.into(),
            primary: false,
            span: span.into(),
        }
    }
}

impl From<Label> for diagnostic::Label<()> {
    fn from(label: Label) -> Self {
        let style = if label.primary {
            diagnostic::LabelStyle::Primary
        } else {
            diagnostic::LabelStyle::Secondary
        };

        diagnostic::Label {
            style,
            file_id: (),
            range: label.span.start()..label.span.end(),
            message: label.message,
        }
    }
}
