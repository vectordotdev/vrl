use super::ExpressionError;
use crate::value::Value;

/// The result of evaluating an expression, including nonlocal control flow.
pub type Resolved<T = Value> = Result<EvaluationOutcome<T>, ExpressionError>;

/// A runtime operation that produces a value without evaluating expressions.
pub type ValueResult = Result<Value, ExpressionError>;

/// Successful evaluation either produces a value or transfers control.
///
/// `T` is normally [`Value`]. Evaluation helpers can use other types (for example,
/// optional argument values) while propagating the same return and break signals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvaluationOutcome<T = Value> {
    /// Continue evaluation with the produced value.
    Value(T),
    /// Stop evaluating the current program or closure.
    Return(Value),
    /// Stop evaluating the innermost breakable loop.
    Break,
}

/// Extract a normal value, propagating errors and nonlocal control flow.
#[macro_export]
macro_rules! resolve_value {
    ($result:expr) => {
        match $result? {
            $crate::compiler::EvaluationOutcome::Value(value) => value,
            $crate::compiler::EvaluationOutcome::Return(value) => {
                return ::std::result::Result::Ok($crate::compiler::EvaluationOutcome::Return(
                    value,
                ));
            }
            $crate::compiler::EvaluationOutcome::Break => {
                return ::std::result::Result::Ok($crate::compiler::EvaluationOutcome::Break);
            }
        }
    };
}
