use std::fmt;

use crate::compiler::expression::Resolved;
use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{Context, Expression, ExpressionError, Span, TypeDef};

#[derive(Debug, Clone, PartialEq)]
pub struct Break {
    pub span: Span,
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
        f.write_str("break")
    }
}

#[derive(Debug)]
pub(crate) struct Error {
    span: Span,
}

impl Error {
    #[must_use]
    pub(crate) const fn new(span: Span) -> Self {
        Self { span }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("break outside of loop")
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl crate::diagnostic::DiagnosticMessage for Error {
    fn code(&self) -> usize {
        crate::compiler::codes::CompilerCode::LoopControlOutsideLoop as usize
    }

    fn labels(&self) -> Vec<crate::diagnostic::Label> {
        vec![crate::diagnostic::Label::primary(
            "break outside of loop",
            self.span,
        )]
    }
}
