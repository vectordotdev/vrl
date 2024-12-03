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
    pub const fn field(value: &str) -> BorrowedSegment {
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

#[cfg(any(test, feature = "arbitrary"))]
impl quickcheck::Arbitrary for BorrowedSegment<'static> {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        if bool::arbitrary(g) {
            if bool::arbitrary(g) {
                BorrowedSegment::Index((usize::arbitrary(g) % 20) as isize)
            } else {
                BorrowedSegment::Index(-((usize::arbitrary(g) % 20) as isize))
            }
        } else {
            BorrowedSegment::Field(String::arbitrary(g).into())
        }
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        match self {
            BorrowedSegment::Invalid => Box::new(std::iter::empty()),
            BorrowedSegment::Index(index) => Box::new(index.shrink().map(BorrowedSegment::Index)),
            BorrowedSegment::Field(field) => Box::new(
                field
                    .to_string()
                    .shrink()
                    .map(|f| BorrowedSegment::Field(f.into())),
            ),
        }
    }
}
