#![deny(warnings)]
#![deny(clippy::all)]
#![deny(unreachable_pub)]
#![deny(unused_allocation)]
#![deny(unused_extern_crates)]
#![deny(unused_assignments)]
#![deny(unused_comparisons)]
#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "compiler")]
pub use vrl_compiler as compiler;

#[cfg(feature = "value")]
pub use value;

#[cfg(feature = "diagnostic")]
pub use vrl_diagnostic as diagnostic;

#[cfg(feature = "path")]
pub use path;

#[cfg(feature = "parser")]
pub use vrl_parser as parser;

#[cfg(feature = "core")]
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
