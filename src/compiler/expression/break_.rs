use std::fmt;

use crate::compiler::{
    Context, Expression, Span, TypeDef, codes,
    expression::Resolved,
    state::{TypeInfo, TypeState},
};
use crate::diagnostic::{DiagnosticMessage, Label, Note};

use super::ExpressionError;

#[derive(Debug, Clone, PartialEq)]
pub struct Break {
    span: Span,
}

impl Break {
    #[must_use]
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}

impl Expression for Break {
    fn resolve(&self, _ctx: &mut Context) -> Resolved {
        Err(ExpressionError::Break { span: self.span })
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        TypeInfo::new(state, TypeDef::never())
    }
}

impl fmt::Display for Break {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "break")
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct Error {
    span: Span,
}

impl Error {
    #[must_use]
    pub fn new(span: Span) -> Self {
        Self { span }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "break outside of loop or iterator")
    }
}

impl std::error::Error for Error {}

impl DiagnosticMessage for Error {
    fn code(&self) -> usize {
        codes::CompilerCode::BreakOutsideLoop as usize
    }

    fn labels(&self) -> Vec<Label> {
        vec![Label::primary(
            "break can only be used inside a loop or iterator",
            self.span,
        )]
    }

    fn notes(&self) -> Vec<Note> {
        vec![Note::SeeErrorDocs]
    }
}
