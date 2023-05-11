use super::ValuePath;

#[derive(Clone)]
pub struct PathConcat<A, B> {
    pub a: A,
    pub b: B,
}

impl<'a, A: ValuePath<'a>, B: ValuePath<'a>> ValuePath<'a> for PathConcat<A, B> {
    type Iter = std::iter::Chain<A::Iter, B::Iter>;

    fn segment_iter(&self) -> Self::Iter {
        self.a.segment_iter().chain(self.b.segment_iter())
    }
}
