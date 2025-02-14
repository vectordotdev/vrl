#![deny(warnings, clippy::pedantic)]

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

/// Compiles the given source code into the final [`Program`].
///
/// This function initializes an external environment and default compilation
/// configuration before invoking the compilation process.
///
/// # Arguments
///
/// * `source` - The source code to be compiled.
/// * `fns` - A list of function definitions available during compilation.
///
/// # Returns
///
/// A `Result` containing the compiled program or a diagnostic error.
///
/// # Errors
///
/// On compilation error, a list of diagnostics is returned.
pub fn compile(source: &str, fns: &[Box<dyn Function>]) -> Result {
    let external = state::ExternalEnv::default();
    let config = CompileConfig::default();

    compile_with_external(source, fns, &external, config)
}

/// Compiles the given source code with a specified external environment and configuration.
///
/// This function allows for customization of the compilation environment by providing
/// an external state and a compilation configuration.
///
/// # Arguments
///
/// * `source` - The source code to be compiled.
/// * `fns` - A list of function definitions available during compilation.
/// * `external` - An external environment providing additional context for compilation.
/// * `config` - The compilation configuration settings.
///
/// # Returns
///
/// A `Result` containing the compiled program or a diagnostic errors.
///
/// # Errors
///
/// On compilation error, a list of diagnostics is returned.
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

/// Compiles the given source code with a specified compilation state and configuration.
///
/// This function performs parsing, compilation, and optional unused expression
/// checking before returning the compilation result.
///
/// # Arguments
///
/// * `source` - The source code to be compiled.
/// * `fns` - A list of function definitions available during compilation.
/// * `state` - The compilation state containing local and external environments.
/// * `config` - The compilation configuration settings.
///
/// # Returns
///
/// A `Result` containing the compiled program or a diagnostic errors.
///
/// # Errors
///
/// On compilation error, a list of diagnostics is returned.
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
