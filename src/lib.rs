// #![deny(warnings)]
#![warn(clippy::all)]
#![warn(unused_allocation)]
#![warn(unused_extern_crates)]
#![warn(unused_assignments)]
#![warn(unused_comparisons)]
#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "compiler")]
pub use compiler::prelude;

#[cfg(feature = "value")]
pub mod value;

#[cfg(feature = "diagnostic")]
pub mod diagnostic;

#[cfg(feature = "path")]
pub mod path;

#[cfg(feature = "parser")]
pub mod parser;

#[cfg(feature = "core")]
pub mod core;

#[cfg(feature = "stdlib")]
pub mod stdlib;

#[cfg(feature = "stdlib")]
pub mod protobuf;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "test_framework")]
pub mod test;

#[cfg(feature = "parsing")]
pub mod parsing;

mod datadog;

#[cfg(feature = "datadog_filter")]
pub use datadog::filter as datadog_filter;

#[cfg(all(feature = "datadog_grok", not(target_arch = "wasm32")))]
pub use datadog::grok as datadog_grok;

#[cfg(feature = "datadog_search")]
pub use datadog::search as datadog_search_syntax;
