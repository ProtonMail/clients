/// This is a utility ergonomic trait as a shorthand for doing
/// `foo.into_iter().map(Into::into).collect::<Vec<_>>()`
pub trait MapVec<A> {
    fn map_vec(self) -> A;
}

impl<T: IntoIterator<Item = B>, A, B> MapVec<Vec<A>> for T
where
    B: Into<A>,
{
    fn map_vec(self) -> Vec<A> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: IntoIterator<Item = B>, A, B> MapVec<Option<Vec<A>>> for Option<T>
where
    B: Into<A>,
{
    fn map_vec(self) -> Option<Vec<A>> {
        self.map(MapVec::map_vec)
    }
}
