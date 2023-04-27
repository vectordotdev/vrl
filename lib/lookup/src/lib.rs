#![deny(warnings)]

pub use error::LookupError;
pub use lookup_v2::{OwnedTargetPath, OwnedValuePath, PathPrefix};

mod error;
pub mod lookup_v2;
