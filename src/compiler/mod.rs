#![deny(warnings, clippy::pedantic)]
#![allow(
clippy::cast_possible_truncation, // allowed in initial deny commit
clippy::cast_possible_wrap, // allowed in initial deny commit
clippy::cast_precision_loss, // allowed in initial deny commit
clippy::cast_sign_loss, // allowed in initial deny commit
clippy::missing_errors_doc, // allowed in initial deny commit
clippy::missing_panics_doc, // allowed in initial deny commit
clippy::module_name_repetitions, // allowed in initial deny commit
)]

use std::fmt::Debug;
use std::{fmt::Display, str::FromStr};

pub use paste::paste;
use serde::{Deserialize, Serialize};

use crate::compiler::unused_expression_checker::check_for_unused_results;
pub use compiler::{CompilationResult, Compiler};
pub use context::Context;
pub use datetime::TimeZone;
pub use expression::{Expression, FunctionExpression};
pub use expression_error::{ExpressionError, Resolved};
pub use function::{Function, Parameter};
pub use program::{Program, ProgramInfo};
pub use state::{TypeInfo, TypeState};
pub use target::{SecretTarget, Target, TargetValue, TargetValueRef};
pub use type_def::TypeDef;

pub(crate) use crate::diagnostic::Span;
use crate::diagnostic::{DiagnosticList, DiagnosticMessage};
use crate::parser::parse;

pub use self::compile_config::CompileConfig;
pub use self::deprecation_warning::DeprecationWarning;

#[allow(clippy::module_inception)]
mod compiler;

mod compile_config;
mod context;
mod datetime;
mod deprecation_warning;
mod expression_error;
mod program;
mod target;
mod test_util;

pub mod codes;
pub mod conversion;
pub mod expression;
pub mod function;
pub mod prelude;
pub mod runtime;
pub mod state;
pub mod type_def;
pub mod unused_expression_checker;
pub mod value;

pub type DiagnosticMessages = Vec<Box<dyn DiagnosticMessage>>;
pub type Result<T = CompilationResult> = std::result::Result<T, DiagnosticList>;

/// Compile a given source into the final [`Program`].
pub fn compile(source: &str, fns: &[Box<dyn Function>]) -> Result {
    let external = state::ExternalEnv::default();
    let config = CompileConfig::default();

    compile_with_external(source, fns, &external, config)
}

pub fn compile_with_external(
    source: &str,
    fns: &[Box<dyn Function>],
    external: &state::ExternalEnv,
    config: CompileConfig,
) -> Result {
    let state = TypeState {
        local: state::LocalEnv::default(),
        external: external.clone(),
    };

    compile_with_state(source, fns, &state, config)
}

pub fn compile_with_state(
    source: &str,
    fns: &[Box<dyn Function>],
    state: &TypeState,
    config: CompileConfig,
) -> Result {
    let ast = parse(source)
        .map_err(|err| crate::diagnostic::DiagnosticList::from(vec![Box::new(err) as Box<_>]))?;

    let unused_expression_check_enabled = config.unused_expression_check_enabled();
    let result = Compiler::compile(fns, ast.clone(), state, config);

    if unused_expression_check_enabled {
        let unused_warnings = check_for_unused_results(&ast);
        if !unused_warnings.is_empty() {
            return result.map(|mut compilation_result| {
                compilation_result.warnings.extend(unused_warnings);
                compilation_result
            });
        }
    }

    result
}

/// Available VRL runtimes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VrlRuntime {
    /// Tree-walking runtime.
    ///
    /// This is the only, and default, runtime.
    Ast,
}

impl Default for VrlRuntime {
    fn default() -> Self {
        Self::Ast
    }
}

impl FromStr for VrlRuntime {
    type Err = &'static str;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "ast" => Ok(Self::Ast),
            _ => Err("runtime must be ast."),
        }
    }
}

impl Display for VrlRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                VrlRuntime::Ast => "ast",
            }
        )
    }
}

/// re-export of commonly used parser types.
pub(crate) mod parser {
    pub(crate) use crate::parser::ast::{self, Ident, Node};
}
