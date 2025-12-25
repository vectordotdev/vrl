use std::fmt;

use crate::compiler::state::{TypeInfo, TypeState};
use crate::compiler::{
    Context, Expression, Span,
    expression::{Expr, Resolved},
    parser::Node,
    value::{Kind, VrlValueArithmetic},
};
use crate::diagnostic::{DiagnosticMessage, Label, Note, Urls};

#[derive(Debug, Clone, PartialEq)]
pub struct BitwiseNot {
    inner: Box<Expr>,
}

pub(crate) type Result = std::result::Result<BitwiseNot, Error>;

impl BitwiseNot {
    /// Creates a new `BitwiseNot` expression.
    ///
    /// # Errors
    /// Returns an `Error` if the provided expression's type is not integer or bytes.
    ///
    /// # Arguments
    /// * `node` - The node representing the expression.
    /// * `not_span` - The span of the `bitwise not` operator.
    /// * `state` - The current type state.
    ///
    /// # Returns
    /// A `Result` containing the new `BitwiseNot` expression or an error.
    ///
    /// # Errors
    /// - `NonInteger`: If operand is not of type integer.
    pub fn new(node: Node<Expr>, not_span: Span, state: &TypeState) -> Result {
        let (expr_span, expr) = node.take();
        let type_def = expr.type_info(state).result;

        if !type_def.is_integer() && !type_def.is_bytes() {
            return Err(Error {
                variant: ErrorVariant::NonInteger(type_def.into()),
                not_span,
                expr_span,
            });
        }

        Ok(Self {
            inner: Box::new(expr),
        })
    }
}

impl Expression for BitwiseNot {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        Ok(self.inner.resolve(ctx)?.try_bitwise_not()?)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let mut state = state.clone();
        let mut inner_def = self.inner.apply_type_info(&mut state);
        if inner_def.is_integer() {
            inner_def = inner_def.infallible().with_kind(Kind::integer());
        } else {
            inner_def = inner_def.fallible().with_kind(Kind::integer());
        }
        TypeInfo::new(state, inner_def)
    }
}

impl fmt::Display for BitwiseNot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "~{}", self.inner)
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug)]
pub struct Error {
    pub(crate) variant: ErrorVariant,

    not_span: Span,
    expr_span: Span,
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum ErrorVariant {
    #[error("non-integer bitwise negation")]
    NonInteger(Kind),
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
        use ErrorVariant::NonInteger;

        match &self.variant {
            NonInteger(..) => 670,
        }
    }

    fn labels(&self) -> Vec<Label> {
        use ErrorVariant::NonInteger;

        match &self.variant {
            NonInteger(kind) => vec![
                Label::primary("bitwise negation only works on integers", self.not_span),
                Label::context(
                    format!("this expression resolves to {kind}"),
                    self.expr_span,
                ),
            ],
        }
    }

    fn notes(&self) -> Vec<Note> {
        use ErrorVariant::NonInteger;

        match &self.variant {
            NonInteger(..) => {
                vec![
                    Note::CoerceValue,
                    Note::SeeDocs(
                        "type coercion".to_owned(),
                        Urls::func_docs("#coerce-functions"),
                    ),
                ]
            }
        }
    }
}

// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    use crate::compiler::{TypeDef, expression::Literal};
    use crate::test_type_def;

    use super::*;

    test_type_def![bitwise_not_integer {
        expr: |_| BitwiseNot {
            inner: Box::new(Literal::from(10).into())
        },
        want: TypeDef::integer().infallible(),
    }];
}
