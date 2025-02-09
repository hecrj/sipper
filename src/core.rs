use crate::{Future, Stream};

/// The core trait of a [`Sipper`].
///
/// It is used internally for convenience to avoid phantom data, since it
/// has [`Output`] and [`Progress`] as associated types.
pub trait Core: Future<Output = <Self as Core>::Output> + Stream<Item = Self::Progress> {
    /// The output of the [`Sipper`].
    type Output;

    /// The progress of the [`Sipper`].
    type Progress;
}

impl<T> Core for T
where
    T: Future + Stream,
{
    type Output = <T as Future>::Output;
    type Progress = T::Item;
}
