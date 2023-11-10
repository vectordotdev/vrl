use crate::diagnostic::{Diagnostic, DiagnosticMessage, Label, Note, Severity};
use crate::value::Value;

pub type Resolved = Result<Value, ExpressionError2>;

// TODO: merge with [`expression::ExpressionError`]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionError2 {
    Abort {
        span: crate::diagnostic::Span,
        message: Option<String>,
    },
    Error {
        message: String,
        labels: Vec<Label>,
        notes: Vec<Note>,
    },
}

impl std::fmt::Display for ExpressionError2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message().fmt(f)
    }
}

impl std::error::Error for ExpressionError2 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<ExpressionError2> for Diagnostic {
    fn from(error: ExpressionError2) -> Self {
        Self {
            severity: Severity::Error,
            code: error.code(),
            message: error.message(),
            labels: error.labels(),
            notes: error.notes(),
        }
    }
}

impl DiagnosticMessage for ExpressionError2 {
    fn code(&self) -> usize {
        0
    }

    fn message(&self) -> String {
        use ExpressionError2::Abort;
        use ExpressionError2::Error;

        match self {
            Abort { message, .. } => message.clone().unwrap_or_else(|| "aborted".to_owned()),
            Error { message, .. } => message.clone(),
        }
    }

    fn labels(&self) -> Vec<Label> {
        use ExpressionError2::Abort;
        use ExpressionError2::Error;

        match self {
            Abort { span, .. } => {
                vec![Label::primary("aborted", span)]
            }
            Error { labels, .. } => labels.clone(),
        }
    }

    fn notes(&self) -> Vec<Note> {
        use ExpressionError2::Abort;
        use ExpressionError2::Error;

        match self {
            Abort { .. } => vec![],
            Error { notes, .. } => notes.clone(),
        }
    }
}

impl From<String> for ExpressionError2 {
    fn from(message: String) -> Self {
        ExpressionError2::Error {
            message,
            labels: vec![],
            notes: vec![],
        }
    }
}

impl From<&str> for ExpressionError2 {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
