//! The `value` crate contains types shared across Vector libraries to support it's use of `Value`
//! and the closely linked `Kind` in support of progressive type checking.

#![deny(warnings, clippy::pedantic)]
#![allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]

pub mod kind;
pub mod secrets;

#[allow(clippy::module_inception)]
pub mod value;

mod btreemap;
mod keystring;

pub use kind::Kind;

pub use self::keystring::KeyString;
pub use self::secrets::Secrets;
#[allow(clippy::module_name_repetitions)]
pub use self::value::{ObjectMap, Value, ValueRegex};

/// A macro to easily generate Values
#[macro_export]
macro_rules! value {
    ([]) => ({
        $crate::value::Value::Array(vec![])
    });

    ([$($v:tt),+ $(,)?]) => ({
        let vec: Vec<$crate::value::Value> = vec![$($crate::value!($v)),+];
        $crate::value::Value::Array(vec)
    });

    ({}) => ({
        $crate::value::Value::Object(::std::collections::BTreeMap::default())
    });

    ({$($($k1:literal)? $($k2:ident)?: $v:tt),+ $(,)?}) => ({
        let map = vec![$((String::from($($k1)? $(stringify!($k2))?).into(), $crate::value!($v))),+]
            .into_iter()
            .collect::<::std::collections::BTreeMap<_, $crate::value::Value>>();

        $crate::value::Value::Object(map)
    });

    (null) => ({
        $crate::value::Value::Null
    });

    ($k:expr) => ({
        $crate::value::Value::from($k)
    });
}
