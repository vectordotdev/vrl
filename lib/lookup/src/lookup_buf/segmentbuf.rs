use std::fmt::{Display, Formatter};

// use inherent::inherent;

// use crate::{field, LookSegment};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct FieldBuf {
    pub name: String,
    // This is a very lazy optimization to avoid having to scan for escapes.
    pub requires_quoting: bool,
}

impl FieldBuf {
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl Display for FieldBuf {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        if self.requires_quoting {
            write!(formatter, r#""{}""#, self.name)
        } else {
            write!(formatter, "{}", self.name)
        }
    }
}

// impl From<String> for FieldBuf {
//     fn from(mut name: String) -> Self {
//         let mut requires_quoting = false;

//         if name.starts_with('\"') && name.ends_with('\"') {
//             // There is unfortunately no way to make an owned substring of a string.
//             // So we have to take a slice and clone it.
//             let len = name.len();
//             name = name[1..len - 1].to_string();
//             requires_quoting = true;
//         } else if !field::is_valid_fieldname(&name) {
//             requires_quoting = true
//         }

//         Self {
//             name,
//             requires_quoting,
//         }
//     }
// }

// impl From<&str> for FieldBuf {
//     fn from(name: &str) -> Self {
//         Self::from(name.to_string())
//     }
// }

/// `SegmentBuf`s are chunks of a `LookupBuf`.
///
/// They represent either a field or an index. A sequence of `SegmentBuf`s can become a `LookupBuf`.
///
/// This is the owned, allocated side of a `Segment` for `LookupBuf.` It owns its fields unlike `Lookup`. Think of `String` to `&str` or `PathBuf` to `Path`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum SegmentBuf {
    Field(FieldBuf),
    Index(isize),
    // Indexes can be negative.
    // Coalesces hold multiple possible fields.
    Coalesce(Vec<FieldBuf>),
}

// #[inherent]
// impl LookSegment for SegmentBuf {
//     type Field = FieldBuf;

//     // pub fn field(field: FieldBuf) -> SegmentBuf {
//     //     SegmentBuf::Field(field)
//     // }

//     // pub fn is_field(&self) -> bool {
//     //     matches!(self, SegmentBuf::Field(_))
//     // }

//     // pub fn index(v: isize) -> SegmentBuf {
//     //     SegmentBuf::Index(v)
//     // }

//     // pub fn is_index(&self) -> bool {
//     //     matches!(self, SegmentBuf::Index(_))
//     // }

//     // pub fn coalesce(v: Vec<FieldBuf>) -> SegmentBuf {
//     //     SegmentBuf::Coalesce(v)
//     // }

//     // pub fn is_coalesce(&self) -> bool {
//     //     matches!(self, SegmentBuf::Coalesce(_))
//     // }
// }
