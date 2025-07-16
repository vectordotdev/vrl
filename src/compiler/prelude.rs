pub use std::fmt;

pub use bytes::Bytes;
pub use indoc::indoc;
pub use ordered_float::NotNan;

// macros
pub use crate::diagnostic::{DiagnosticMessage, Note, Span};
pub use crate::expr;
pub use crate::value::{
    KeyString, Kind, ObjectMap, Value, ValueRegex,
    kind::{Collection, Field, Index},
    value,
    value::IterItem,
};
#[cfg(any(test, feature = "test"))]
pub use crate::{func_args, test_function, test_type_def};

pub use super::Resolved;
pub use super::{
    Context, Expression, ExpressionError, Function, FunctionExpression, Parameter, TimeZone,
    TypeDef, expression,
    function::{self, ArgumentList, Closure, Compiled, Example, FunctionCompileContext, closure},
    state::{self, TypeInfo, TypeState},
    type_def,
    value::{ValueError, VrlValueArithmetic, VrlValueConvert, kind},
};

pub type ExpressionResult<T> = Result<T, ExpressionError>;
