// Central registry for all VRL diagnostic error codes.
// Each code maps to documentation at https://errors.vrl.dev/{code}.
//
// All discriminants are set explicitly to preserve stable values.
// When adding a new code: pick the next unused value in the appropriate range
// and document it here. Never reuse a code that appeared in a release.

/// Expression, assignment, and function-call errors (100–)
#[repr(usize)]
pub enum ExprCode {
    FallibleExpression = 100,
    InvalidRegex = 101,
    NonBooleanPredicate = 102,
    FallibleAssignment = 103,
    InfallibleAssignment = 104,
    UndefinedFunction = 105,
    WrongNumberOfArgs = 106,
    MissingArgument = 107,
    UnknownKeyword = 108,
    UnexpectedClosure = 109,
    InvalidArgumentKind = 110,
    MissingClosure = 111,
    InvalidGrokPattern = 112,
    NonStringAbortMessage = 113,
    ExpressionTypeUnavailable = 114,
    ClosureArityMismatch = 120,
    ClosureParameterTypeMismatch = 121,
    ReturnTypeMismatch = 122,
}

/// Parser and lexer errors (200–)
#[repr(usize)]
pub enum ParserCode {
    InvalidToken = 200,
    ExtraToken = 201,
    User = 202,
    UnrecognizedToken = 203,
    UnrecognizedEof = 204,
    ReservedKeyword = 205,
    NumericLiteral = 206,
    StringLiteral = 207,
    Literal = 208,
    EscapeChar = 209,
    UnexpectedParse = 210,
    UnicodeEscape = 211,
}

/// Value and runtime operation errors (300–)
#[repr(usize)]
pub enum ValueCode {
    ExpectedType = 300,
    Coerce = 301,
    Remainder = 302,
    Multiply = 303,
    Divide = 304,
    DivideByZero = 305,
    NanFloat = 306,
    Add = 307,
    Subtract = 308,
    Or = 309,
    And = 310,
    GreaterThan = 311,
    GreaterThanOrEqual = 312,
    LessThan = 313,
    LessThanOrEqual = 314,
    /// Mutation attempted on a read-only value (merge, assignment, function argument).
    ReadOnlyMutation = 315,
    OutOfRange = 316,
}

/// Function-argument compilation errors (400–)
#[repr(usize)]
pub enum FunctionCode {
    UnexpectedExpression = 400,
    InvalidEnumVariant = 401,
    ExpectedStaticExpression = 402,
    InvalidArgument = 403,
    ExpectedFunctionClosure = 420,
}

/// Compiler and type-checking errors (600–)
#[repr(usize)]
pub enum CompilerCode {
    InvalidTimestamp = 601,
    NanFloatLiteral = 602,
    FunctionCompilation = 610,
    AbortInfallible = 620,
    FallibleArgument = 630,
    /// Fallible expression used where only infallible is allowed (abort/return message).
    FallibleExpr = 631,
    UnnecessaryNoop = 640,
    InvalidTarget = 641,
    InvalidParentPathSegment = 642,
    ChainedComparison = 650,
    UnnecessaryCoalesce = 651,
    MergeNonObjects = 652,
    NonBooleanNot = 660,
}

/// Variable errors (700–)
#[repr(usize)]
pub enum VariableCode {
    UndefinedVariable = 701,
}

/// Warnings (800–)
#[repr(usize)]
pub enum WarningCode {
    Deprecation = 801,
    UnusedCode = 900,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // Expands to:
    //   1. An exhaustive match closure — adding a variant to the enum causes a compile error here.
    //   2. A slice of all discriminant values fed into the uniqueness check.
    macro_rules! exhaustive_codes {
        ($enum_ty:ty, $($variant:path),+ $(,)?) => {{
            let _ = |v: $enum_ty| match v { $($variant => (),)+ };
            [$($variant as usize,)+]
        }};
    }

    #[test]
    fn codes_are_unique() {
        let all: &[&[usize]] = &[
            &exhaustive_codes!(
                ExprCode,
                ExprCode::FallibleExpression,
                ExprCode::InvalidRegex,
                ExprCode::NonBooleanPredicate,
                ExprCode::FallibleAssignment,
                ExprCode::InfallibleAssignment,
                ExprCode::UndefinedFunction,
                ExprCode::WrongNumberOfArgs,
                ExprCode::MissingArgument,
                ExprCode::UnknownKeyword,
                ExprCode::UnexpectedClosure,
                ExprCode::InvalidArgumentKind,
                ExprCode::MissingClosure,
                ExprCode::InvalidGrokPattern,
                ExprCode::NonStringAbortMessage,
                ExprCode::ExpressionTypeUnavailable,
                ExprCode::ClosureArityMismatch,
                ExprCode::ClosureParameterTypeMismatch,
                ExprCode::ReturnTypeMismatch,
            ),
            &exhaustive_codes!(
                ParserCode,
                ParserCode::InvalidToken,
                ParserCode::ExtraToken,
                ParserCode::User,
                ParserCode::UnrecognizedToken,
                ParserCode::UnrecognizedEof,
                ParserCode::ReservedKeyword,
                ParserCode::NumericLiteral,
                ParserCode::StringLiteral,
                ParserCode::Literal,
                ParserCode::EscapeChar,
                ParserCode::UnexpectedParse,
                ParserCode::UnicodeEscape,
            ),
            &exhaustive_codes!(
                ValueCode,
                ValueCode::ExpectedType,
                ValueCode::Coerce,
                ValueCode::Remainder,
                ValueCode::Multiply,
                ValueCode::Divide,
                ValueCode::DivideByZero,
                ValueCode::NanFloat,
                ValueCode::Add,
                ValueCode::Subtract,
                ValueCode::Or,
                ValueCode::And,
                ValueCode::GreaterThan,
                ValueCode::GreaterThanOrEqual,
                ValueCode::LessThan,
                ValueCode::LessThanOrEqual,
                ValueCode::ReadOnlyMutation,
                ValueCode::OutOfRange,
            ),
            &exhaustive_codes!(
                FunctionCode,
                FunctionCode::UnexpectedExpression,
                FunctionCode::InvalidEnumVariant,
                FunctionCode::ExpectedStaticExpression,
                FunctionCode::InvalidArgument,
                FunctionCode::ExpectedFunctionClosure,
            ),
            &exhaustive_codes!(
                CompilerCode,
                CompilerCode::InvalidTimestamp,
                CompilerCode::NanFloatLiteral,
                CompilerCode::FunctionCompilation,
                CompilerCode::AbortInfallible,
                CompilerCode::FallibleArgument,
                CompilerCode::FallibleExpr,
                CompilerCode::UnnecessaryNoop,
                CompilerCode::InvalidTarget,
                CompilerCode::InvalidParentPathSegment,
                CompilerCode::ChainedComparison,
                CompilerCode::UnnecessaryCoalesce,
                CompilerCode::MergeNonObjects,
                CompilerCode::NonBooleanNot,
            ),
            &exhaustive_codes!(VariableCode, VariableCode::UndefinedVariable,),
            &exhaustive_codes!(
                WarningCode,
                WarningCode::Deprecation,
                WarningCode::UnusedCode,
            ),
        ];

        let mut seen = HashSet::new();
        for &code in all.iter().flat_map(|s| s.iter()) {
            assert!(seen.insert(code), "duplicate error code: {code}");
        }
    }
}
