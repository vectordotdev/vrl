use super::Kind;
use crate::compiler::ExpressionError;
use crate::compiler::codes;
use crate::diagnostic::DiagnosticMessage;
use crate::prelude::ValueError::OutOfRange;

#[allow(clippy::module_name_repetitions)]
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ValueError {
    #[error(
        "expected {}, got {got}",
        .expected
    )]
    Expected { got: Kind, expected: Kind },

    #[error("can't coerce {0} into {1}")]
    Coerce(Kind, Kind),

    #[error("can't calculate remainder of type {0} and {1}")]
    Rem(Kind, Kind),

    #[error("can't multiply type {0} by {1}")]
    Mul(Kind, Kind),

    #[error("can't divide type {0} by {1}")]
    Div(Kind, Kind),

    #[error("can't divide by zero")]
    DivideByZero,

    #[error("floats can't be NaN")]
    NanFloat,

    #[error("can't add type {1} to {0}")]
    Add(Kind, Kind),

    #[error("can't subtract type {1} from {0}")]
    Sub(Kind, Kind),

    #[error("can't apply an OR to these types - {0}")]
    Or(#[from] ExpressionError),

    #[error("can't apply an AND to types {0} and {1}")]
    And(Kind, Kind),

    #[error("can't compare {0} > {1}")]
    Gt(Kind, Kind),

    #[error("can't compare {0} >= {1}")]
    Ge(Kind, Kind),

    #[error("can't compare {0} < {1}")]
    Lt(Kind, Kind),

    #[error("can't compare {0} <= {1}")]
    Le(Kind, Kind),

    #[error("can't merge type {1} into {0}")]
    Merge(Kind, Kind),

    #[error("can't convert out of range {0}")]
    OutOfRange(Kind),
}

impl DiagnosticMessage for ValueError {
    fn code(&self) -> usize {
        use ValueError::{
            Add, And, Coerce, Div, DivideByZero, Expected, Ge, Gt, Le, Lt, Merge, Mul, NanFloat,
            Or, Rem, Sub,
        };

        match self {
            Expected { .. } => codes::ValueCode::ExpectedType as usize,
            Coerce(..) => codes::ValueCode::Coerce as usize,
            Rem(..) => codes::ValueCode::Remainder as usize,
            Mul(..) => codes::ValueCode::Multiply as usize,
            Div(..) => codes::ValueCode::Divide as usize,
            DivideByZero => codes::ValueCode::DivideByZero as usize,
            NanFloat => codes::ValueCode::NanFloat as usize,
            Add(..) => codes::ValueCode::Add as usize,
            Sub(..) => codes::ValueCode::Subtract as usize,
            Or(..) => codes::ValueCode::Or as usize,
            And(..) => codes::ValueCode::And as usize,
            Gt(..) => codes::ValueCode::GreaterThan as usize,
            Ge(..) => codes::ValueCode::GreaterThanOrEqual as usize,
            Lt(..) => codes::ValueCode::LessThan as usize,
            Le(..) => codes::ValueCode::LessThanOrEqual as usize,
            Merge(..) => codes::ValueCode::ReadOnlyMutation as usize,
            OutOfRange(..) => codes::ValueCode::OutOfRange as usize,
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
