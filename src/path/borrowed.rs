use std::borrow::Cow;
use std::iter::Cloned;
use std::slice::Iter;

use super::{OwnedSegment, PathPrefix, TargetPath, ValuePath};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedValuePath<'a, 'b> {
    pub segments: &'b [BorrowedSegment<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorrowedTargetPath<'a, 'b> {
    pub prefix: PathPrefix,
    pub path: BorrowedValuePath<'a, 'b>,
}

impl<'a, 'b> BorrowedTargetPath<'a, 'b> {
    pub fn event(path: BorrowedValuePath<'a, 'b>) -> Self {
        Self {
            prefix: PathPrefix::Event,
            path,
        }
    }

    pub fn metadata(path: BorrowedValuePath<'a, 'b>) -> Self {
        Self {
            prefix: PathPrefix::Metadata,
            path,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BorrowedSegment<'a> {
    Field(Cow<'a, str>),
    Index(isize),
    Invalid,
}

impl BorrowedSegment<'_> {
    pub const fn field(value: &str) -> BorrowedSegment<'_> {
        BorrowedSegment::Field(Cow::Borrowed(value))
    }
    pub fn index(value: isize) -> BorrowedSegment<'static> {
        BorrowedSegment::Index(value)
    }
    pub fn is_field(&self) -> bool {
        matches!(self, BorrowedSegment::Field(_))
    }
    pub fn is_index(&self) -> bool {
        matches!(self, BorrowedSegment::Index(_))
    }
    pub fn is_invalid(&self) -> bool {
        matches!(self, BorrowedSegment::Invalid)
    }
}

impl<'a> From<&'a OwnedSegment> for BorrowedSegment<'a> {
    fn from(segment: &'a OwnedSegment) -> Self {
        match segment {
            OwnedSegment::Field(field) => Self::Field(field.as_str().into()),
            OwnedSegment::Index(i) => Self::Index(*i),
        }
    }
}

impl<'a> From<&'a str> for BorrowedSegment<'a> {
    fn from(field: &'a str) -> Self {
        BorrowedSegment::field(field)
    }
}

impl<'a> From<&'a String> for BorrowedSegment<'a> {
    fn from(field: &'a String) -> Self {
        BorrowedSegment::field(field.as_str())
    }
}

impl From<isize> for BorrowedSegment<'_> {
    fn from(index: isize) -> Self {
        BorrowedSegment::index(index)
    }
}

impl<'a, 'b> ValuePath<'a> for BorrowedValuePath<'a, 'b> {
    type Iter = Cloned<Iter<'b, BorrowedSegment<'a>>>;

    fn segment_iter(&self) -> Self::Iter {
        self.segments.iter().cloned()
    }
}

impl<'a, 'b> ValuePath<'a> for &'b Vec<BorrowedSegment<'a>> {
    type Iter = Cloned<Iter<'b, BorrowedSegment<'a>>>;

    fn segment_iter(&self) -> Self::Iter {
        self.as_slice().iter().cloned()
    }
}

impl<'a, 'b> TargetPath<'a> for BorrowedTargetPath<'a, 'b> {
    type ValuePath = BorrowedValuePath<'a, 'b>;

    fn prefix(&self) -> PathPrefix {
        self.prefix
    }

    fn value_path(&self) -> Self::ValuePath {
        self.path
    }
}

#[cfg(any(test, feature = "proptest"))]
impl proptest::arbitrary::Arbitrary for BorrowedSegment<'static> {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::prelude::*;
        prop_oneof![
            any::<String>().prop_map(|s| BorrowedSegment::Field(s.into())),
            (-19isize..=19isize).prop_map(BorrowedSegment::Index),
        ]
        .boxed()
    }
}
