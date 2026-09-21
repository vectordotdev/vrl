use ExpressionError::{Abort, ControlFlow, Error, Fallible, Missing};

use crate::compiler::codes;
use crate::diagnostic::{Diagnostic, DiagnosticMessage, Label, Note, Severity, Span};
use crate::value::Value;

pub type Resolved = Result<Value, ExpressionError>;

/// Control signals that travel through evaluation using ordinary `?` propagation.
/// They are not runtime errors and must bypass error recovery and error wrapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlSignal {
    /// Stop execution at an embedder-provided checkpoint.
    Interrupted,
    Return {
        span: Span,
        value: Value,
    },
    Break {
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionError {
    /// Nonlocal control flow, propagated through evaluation without error recovery.
    ControlFlow(ControlSignal),

    Abort {
        span: Span,
        message: Option<String>,
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
            ControlFlow(_) | Abort { .. } | Error { .. } => 0,
            Fallible { .. } => codes::ExprCode::FallibleExpression as usize,
            Missing { .. } => codes::ExprCode::ExpressionTypeUnavailable as usize,
        }
    }

    fn message(&self) -> String {
        match self {
            ControlFlow(ControlSignal::Interrupted) => "execution interrupted".to_owned(),
            Abort { message, .. } => message.clone().unwrap_or_else(|| "aborted".to_owned()),
            ControlFlow(ControlSignal::Return { .. }) => "return".to_string(),
            ControlFlow(ControlSignal::Break { .. }) => "break".to_string(),
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
            ControlFlow(_) => Vec::new(),
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
            ControlFlow(_) | Abort { .. } | Missing { .. } => vec![],
            Error { notes, .. } => notes.clone(),
            Fallible { .. } => vec![Note::SeeErrorDocs],
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
