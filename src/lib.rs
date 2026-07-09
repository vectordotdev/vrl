#![deny(warnings)]
#![warn(clippy::all)]
#![warn(clippy::arithmetic_side_effects)]

#[cfg(feature = "value")]
pub mod value;

#[cfg(feature = "value")]
pub mod path;

#[cfg(feature = "compiler")]
pub mod compiler;

#[cfg(feature = "compiler")]
pub use compiler::prelude;

#[cfg(feature = "compiler")]
pub mod diagnostic;

#[cfg(feature = "compiler")]
pub mod parser;

#[cfg(feature = "stdlib-base")]
pub mod core;

#[cfg(feature = "stdlib-base")]
pub mod stdlib;

#[cfg(feature = "stdlib-base")]
pub mod protobuf;

#[cfg(any(feature = "stdlib-base", feature = "datadog"))]
pub mod parsing;

#[cfg(feature = "docs")]
pub mod docs;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "test_framework")]
pub mod test;

mod datadog;

#[cfg(feature = "datadog")]
pub use datadog::filter as datadog_filter;

#[cfg(all(feature = "datadog", not(target_arch = "wasm32")))]
pub use datadog::grok as datadog_grok;

#[cfg(feature = "datadog")]
pub use datadog::search as datadog_search_syntax;
