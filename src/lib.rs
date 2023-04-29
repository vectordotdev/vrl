#![deny(warnings)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(unused_allocation)]
#![deny(unused_extern_crates)]
#![deny(unused_assignments)]
#![deny(unused_comparisons)]
#![allow(clippy::module_name_repetitions)]

// pub mod prelude;
// pub use compiler::expression::query;
// pub use compiler::{
//     compile, compile_with_external, compile_with_state, function,
//     runtime::{Runtime, RuntimeResult, Terminate},
//     state, value, CompilationResult, CompileConfig, Compiler, Context, Expression, Function,
//     Program, ProgramInfo, SecretTarget, Target, TargetValue, TargetValueRef, VrlRuntime,
// };
// pub use diagnostic;
// pub use vrl_core::TimeZone;

#[cfg(feature = "compiler")]
pub use vrl_compiler as compiler;

#[cfg(feature = "value")]
pub use value;

#[cfg(feature = "diagnostic")]
pub use vrl_diagnostic as diagnostic;

#[cfg(feature = "path")]
pub use lookup::lookup_v2 as path;

#[cfg(feature = "parser")]
pub use vrl_parser as parser;

#[cfg(feature = "stdlib")]
pub use vrl_core as core;
#[cfg(feature = "stdlib")]
pub use vrl_stdlib as stdlib;

#[cfg(feature = "cli")]
pub use vrl_cli as cli;

#[cfg(feature = "test_framework")]
pub use vrl_tests as test;

#[cfg(feature = "datadog_filter")]
pub use datadog_filter;

#[cfg(feature = "datadog_grok")]
pub use datadog_grok;

#[cfg(feature = "datadog_search")]
pub use datadog_search_syntax;
