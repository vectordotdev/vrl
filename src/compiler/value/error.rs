use super::Kind;
use crate::compiler::ExpressionError;
use crate::diagnostic::DiagnosticMessage;
use crate::prelude::ValueError::OutOfRange;

#[allow(clippy::module_name_repetitions)]
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ValueError {
    #[error(
        "expected {}, got {got}",
        .expected
    )]
    Expected { got: Box<Kind>, expected: Box<Kind> },

    #[error("can't coerce {0} into {1}")]
    Coerce(Box<Kind>, Box<Kind>),

    #[error("can't calculate remainder of type {0} and {1}")]
    Rem(Box<Kind>, Box<Kind>),

    #[error("can't multiply type {0} by {1}")]
    Mul(Box<Kind>, Box<Kind>),

    #[error("can't divide type {0} by {1}")]
    Div(Box<Kind>, Box<Kind>),

    #[error("can't divide by zero")]
    DivideByZero,

    #[error("floats can't be NaN")]
    NanFloat,

    #[error("can't add type {1} to {0}")]
    Add(Box<Kind>, Box<Kind>),

    #[error("can't subtract type {1} from {0}")]
    Sub(Box<Kind>, Box<Kind>),

    #[error("can't apply an OR to these types - {0}")]
    Or(#[from] ExpressionError),

    #[error("can't apply an AND to types {0} and {1}")]
    And(Box<Kind>, Box<Kind>),

    #[error("can't compare {0} > {1}")]
    Gt(Box<Kind>, Box<Kind>),

    #[error("can't compare {0} >= {1}")]
    Ge(Box<Kind>, Box<Kind>),

    #[error("can't compare {0} < {1}")]
    Lt(Box<Kind>, Box<Kind>),

    #[error("can't compare {0} <= {1}")]
    Le(Box<Kind>, Box<Kind>),

    #[error("can't merge type {1} into {0}")]
    Merge(Box<Kind>, Box<Kind>),

    #[error("can't convert out of range {0}")]
    OutOfRange(Box<Kind>),
}

impl DiagnosticMessage for ValueError {
    fn code(&self) -> usize {
        use ValueError::{
            Add, And, Coerce, Div, DivideByZero, Expected, Ge, Gt, Le, Lt, Merge, Mul, NanFloat,
            Or, Rem, Sub,
        };

        match self {
            Expected { .. } => 300,
            Coerce(..) => 301,
            Rem(..) => 302,
            Mul(..) => 303,
            Div(..) => 304,
            DivideByZero => 305,
            NanFloat => 306,
            Add(..) => 307,
            Sub(..) => 308,
            Or(..) => 309,
            And(..) => 310,
            Gt(..) => 311,
            Ge(..) => 312,
            Lt(..) => 313,
            Le(..) => 314,
            Merge(..) => 315,
            OutOfRange(..) => 316,
        }
    }
}

impl From<ValueError> for ExpressionError {
    fn from(err: ValueError) -> Self {
        Self::Error {
            message: err.message(),
            labels: vec![],
            notes: vec![],
        }
    }
}
