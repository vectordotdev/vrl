//! The `value` crate contains types shared across Vector libraries to support it's use of `Value`
//! and the closely linked `Kind` in support of progressive type checking.

#![deny(warnings)]
#![deny(
    clippy::all,
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    future_incompatible,
    missing_docs,
    nonstandard_style,
    rust_2018_compatibility,
    rust_2018_idioms,
    rust_2021_compatibility,
    rustdoc::bare_urls,
    rustdoc::broken_intra_doc_links,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_rust_codeblocks,
    rustdoc::missing_crate_level_docs,
    rustdoc::private_doc_tests,
    rustdoc::private_intra_doc_links,
    unused
)]
#![allow(
    deprecated,
    clippy::cast_lossless,
    clippy::cargo_common_metadata,
    clippy::single_match_else,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::module_name_repetitions,
    clippy::missing_const_for_fn,
    clippy::multiple_crate_versions,
    clippy::fallible_impl_from,
    unreachable_code,
    unused_variables
)]

pub mod kind;
pub mod secrets;
pub mod value;

mod btreemap;

pub use kind::Kind;

pub use self::secrets::Secrets;
pub use self::value::{Value, ValueRegex};

/// A macro to easily generate Values
#[macro_export]
macro_rules! value {
    ([]) => ({
        $crate::Value::Array(vec![])
    });

    ([$($v:tt),+ $(,)?]) => ({
        let vec: Vec<$crate::Value> = vec![$($crate::value!($v)),+];
        $crate::Value::Array(vec)
    });

    ({}) => ({
        $crate::Value::Object(::std::collections::BTreeMap::default())
    });

    ({$($($k1:literal)? $($k2:ident)?: $v:tt),+ $(,)?}) => ({
        let map = vec![$((String::from($($k1)? $(stringify!($k2))?), $crate::value!($v))),+]
            .into_iter()
            .collect::<::std::collections::BTreeMap<_, $crate::Value>>();

        $crate::Value::Object(map)
    });

    (null) => ({
        $crate::Value::Null
    });

    ($k:expr) => ({
        $crate::Value::from($k)
    });
}
