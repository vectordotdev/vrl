use ExpressionError::{Abort, Error, Fallible, Missing, Return};

use crate::diagnostic::{Diagnostic, DiagnosticMessage, Label, Note, Severity, Span};
use crate::value::Value;

pub type Resolved = Result<Value, ExpressionError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionError {
    Abort {
        span: Span,
        message: Option<String>,
    },
    Return {
        span: Span,
        value: Value,
    },
    Error {
        message: String,
        labels: Vec<Label>,
        notes: Vec<Note>,
    },

    Fallible {
        span: Span,
    },

    Missing {
        span: Span,
        feature: &'static str,
    },
}

impl std::fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message().fmt(f)
    }
}

impl std::error::Error for ExpressionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<ExpressionError> for Diagnostic {
    fn from(error: ExpressionError) -> Self {
        Self {
            severity: Severity::Error,
            code: error.code(),
            message: error.message(),
            labels: error.labels(),
            notes: error.notes(),
        }
    }
}

impl DiagnosticMessage for ExpressionError {
    fn code(&self) -> usize {
        match self {
            Abort { .. } => 0,
            Return { .. } => 0,
            Error { .. } => 0,
            Fallible { .. } => 100,
            Missing { .. } => 900,
        }
    }

    fn message(&self) -> String {
        match self {
            Abort { message, .. } => message.clone().unwrap_or_else(|| "aborted".to_owned()),
            Return { .. } => "return".to_string(),
            Error { message, .. } => message.clone(),
            Fallible { .. } => "unhandled error".to_string(),
            Missing { .. } => "expression type unavailable".to_string(),
        }
    }

    fn labels(&self) -> Vec<Label> {
        match self {
            Abort { span, .. } => {
                vec![Label::primary("aborted", span)]
            }
            Return { .. } => Vec::new(),
            Error { labels, .. } => labels.clone(),
            Fallible { span } => vec![
                Label::primary("expression can result in runtime error", span),
                Label::context("handle the error case to ensure runtime success", span),
            ],
            Missing { span, feature } => vec![
                Label::primary("expression type is disabled in this version of vrl", span),
                Label::context(
                    format!("build vrl using the `{feature}` feature to enable it"),
                    span,
                ),
            ],
        }
    }

    fn notes(&self) -> Vec<Note> {
        match self {
            Abort { .. } => vec![],
            Return { .. } => vec![],
            Error { notes, .. } => notes.clone(),
            Fallible { .. } => vec![Note::SeeErrorDocs],
            Missing { .. } => vec![],
        }
    }
}

impl From<String> for ExpressionError {
    fn from(message: String) -> Self {
        ExpressionError::Error {
            message,
            labels: vec![],
            notes: vec![],
        }
    }
}

impl From<&str> for ExpressionError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}
