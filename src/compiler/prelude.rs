pub use std::fmt;

pub use bytes::Bytes;
pub use indoc::indoc;
pub use ordered_float::NotNan;

// macros
pub use crate::diagnostic::{DiagnosticMessage, Note, Span};
pub use crate::expr;
pub use crate::value::{
    kind::{Collection, Field, Index},
    value,
    value::IterItem,
    KeyString, Kind, ObjectMap, Value, ValueRegex,
};
#[cfg(any(test, feature = "test"))]
pub use crate::{func_args, test_function, test_type_def};

pub use super::Resolved;
pub use super::{
    expression,
    function::{self, closure, ArgumentList, Closure, Compiled, Example, FunctionCompileContext},
    state::{self, TypeInfo, TypeState},
    type_def,
    value::{kind, ValueError, VrlValueArithmetic, VrlValueConvert},
    Context, Expression, ExpressionError, Function, FunctionExpression, Parameter, TimeZone,
    TypeDef,
};

pub type ExpressionResult<T> = Result<T, ExpressionError>;
