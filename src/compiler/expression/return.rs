use std::fmt;

use crate::compiler::{
    expression::Resolved,
    state::{TypeInfo, TypeState},
    Context, Expression, Span, TypeDef,
};
use crate::diagnostic::{DiagnosticMessage, Label, Note};
use crate::parser::ast::Node;

use super::{Expr, ExpressionError};

#[derive(Debug, Clone, PartialEq)]
pub struct Return {
    span: Span,
    expr: Box<Expr>,
}

impl Return {
    /// # Errors
    ///
    /// * The returned value must not be fallible
    pub fn new(span: Span, expr: Node<Expr>, state: &TypeState) -> Result<Self, Error> {
        let (expr_span, expr) = expr.take();
        let type_def = expr.type_info(state).result;

        if type_def.is_fallible() {
            return Err(Error {
                variant: ErrorVariant::FallibleExpr,
                expr_span,
            });
        }

        Ok(Self {
            span,
            expr: Box::new(expr),
        })
    }
}

impl Expression for Return {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        Err(ExpressionError::Return {
            span: self.span,
            value: self.expr.resolve(ctx)?,
        })
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let value = self.expr.type_info(state);
        TypeInfo::new(
            state,
            TypeDef::never().with_returns(value.result.kind().clone()),
        )
    }
}

impl fmt::Display for Return {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "return")
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct Error {
    variant: ErrorVariant,
    expr_span: Span,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ErrorVariant {
    #[error("unhandled fallible expression")]
    FallibleExpr,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#}", self.variant)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.variant)
    }
}

impl DiagnosticMessage for Error {
    fn code(&self) -> usize {
        use ErrorVariant::FallibleExpr;

        match self.variant {
            FallibleExpr => 631,
        }
    }

    fn labels(&self) -> Vec<Label> {
        match &self.variant {
            ErrorVariant::FallibleExpr => vec![
                Label::primary(
                    "return only accepts an infallible expression argument",
                    self.expr_span,
                ),
                Label::context(
                    "handle errors before using the expression as a return value",
                    self.expr_span,
                ),
            ],
        }
    }

    fn notes(&self) -> Vec<Note> {
        match self.variant {
            ErrorVariant::FallibleExpr => vec![Note::SeeErrorDocs],
        }
    }
}
