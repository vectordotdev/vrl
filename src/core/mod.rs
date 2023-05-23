#![deny(
    warnings,
    clippy::all,
    clippy::pedantic,
    unreachable_pub,
    unused_allocation,
    unused_extern_crates,
    unused_assignments,
    unused_comparisons
)]
#![allow(
    clippy::missing_errors_doc, // allowed in initial deny commit
    clippy::module_name_repetitions, // allowed in initial deny commit
)]

pub mod encode_key_value;
pub mod encode_logfmt;
pub mod tokenize;
pub use crate::value::Value;
