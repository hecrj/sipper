use crate::{Future, Stream};

/// The core trait of a [`Sipper`].
///
/// It is used internally for convenience to avoid phantom data, since it
/// has [`Output`](Self::Output) and [`Progress`](Self::Progress) as associated types.
///
/// [`Sipper`]: crate::Sipper
pub trait Core: Future<Output = <Self as Core>::Output> + Stream<Item = Self::Progress> {
    /// The output of the [`Sipper`].
    ///
    /// [`Sipper`]: crate::Sipper
    type Output;

    /// The progress of the [`Sipper`].
    ///
    /// [`Sipper`]: crate::Sipper
    type Progress;
}

impl<T> Core for T
where
    T: Future + Stream,
{
    type Output = <T as Future>::Output;
    type Progress = T::Item;
}
