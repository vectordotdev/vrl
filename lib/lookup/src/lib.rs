#![deny(warnings)]

// use std::{fmt::Debug, hash::Hash};

pub use error::LookupError;
pub use lookup_buf::{FieldBuf, LookupBuf, SegmentBuf};
pub use lookup_v2::{OwnedTargetPath, OwnedValuePath, PathPrefix};
pub use lookup_view::{Field, Lookup, Segment};

mod error;
mod field;
mod lookup_buf;
pub mod lookup_v2;
mod lookup_view;

// // This trait, while it is not necessarily imported and used, exists
// // to enforce parity among view/buf types.
// //
// // It is convention to implement these functions on the bare type itself,
// // then have the implementation proxy to this **without modification**.
// //
// // This is so the functions are always available to users, without needing an import.
// pub trait LookSegment<'a> {
//     type Field: Debug + PartialEq + Eq + PartialOrd + Ord + Clone + Hash + Sized;

//     // fn field(field: Self::Field) -> Self;

//     // fn is_field(&self) -> bool;

//     // fn index(v: isize) -> Self;

//     // fn is_index(&self) -> bool;

//     // fn coalesce(v: Vec<Self::Field>) -> Self;

//     // fn is_coalesce(&self) -> bool;
// }
