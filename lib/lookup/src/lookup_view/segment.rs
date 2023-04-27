// use inherent::inherent;

use crate::FieldBuf;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Field<'a> {
    pub name: &'a str,
    // This is a very lazy optimization to avoid having to scan for escapes.
    pub requires_quoting: bool,
}

impl<'a> Field<'a> {
    pub fn as_field_buf(&self) -> FieldBuf {
        FieldBuf {
            name: self.name.to_string(),
            requires_quoting: self.requires_quoting,
        }
    }
}

impl<'a> From<&'a FieldBuf> for Field<'a> {
    fn from(v: &'a FieldBuf) -> Self {
        Self {
            name: &v.name,
            requires_quoting: v.requires_quoting,
        }
    }
}

/// Segments are chunks of a lookup. They represent either a field or an index.
/// A sequence of Segments can become a lookup.
///
/// If you need an owned, allocated version, see `SegmentBuf`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub enum Segment<'a> {
    Field(Field<'a>),
    Index(isize),
    // Coalesces hold multiple possible fields.
    Coalesce(Vec<Field<'a>>),
}

impl<'a> Segment<'a> {
    // pub fn as_segment_buf(&self) -> SegmentBuf {
    //     match self {
    //         Segment::Field(field) => SegmentBuf::field(field.as_field_buf()),
    //         Segment::Index(i) => SegmentBuf::index(*i),
    //         Segment::Coalesce(v) => {
    //             SegmentBuf::coalesce(v.iter().map(|field| field.as_field_buf()).collect())
    //         }
    //     }
    // }
}

// #[inherent]
// impl<'a> LookSegment<'a> for Segment<'a> {
//     type Field = Field<'a>;

//     // pub fn field(field: Field<'a>) -> Segment<'a> {
//     //     Segment::Field(field)
//     // }

//     // pub fn is_field(&self) -> bool {
//     //     matches!(self, Segment::Field(_))
//     // }

//     // pub fn index(v: isize) -> Segment<'a> {
//     //     Segment::Index(v)
//     // }

//     // pub fn is_index(&self) -> bool {
//     //     matches!(self, Segment::Index(_))
//     // }

//     // pub fn coalesce(v: Vec<Field<'a>>) -> Segment<'a> {
//     //     Segment::Coalesce(v)
//     // }

//     // pub fn is_coalesce(&self) -> bool {
//     //     matches!(self, Segment::Coalesce(_))
//     // }
// }
