#![deny(warnings)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(unused_allocation)]
#![deny(unused_extern_crates)]
#![deny(unused_assignments)]
#![deny(unused_comparisons)]
#![allow(clippy::module_name_repetitions)]

pub mod prelude;
pub use compiler::expression::query;
pub use compiler::{
    compile, compile_with_external, compile_with_state, function,
    runtime::{Runtime, RuntimeResult, Terminate},
    state, value, CompilationResult, CompileConfig, Compiler, Context, Expression, Function,
    Program, ProgramInfo, SecretTarget, Target, TargetValue, TargetValueRef, VrlRuntime,
};
pub use diagnostic;
pub use vrl_core::TimeZone;
