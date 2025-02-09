use crate::{Never, Sipper};

/// A [`Straw`] is a [`Sipper`] that can fail.
///
/// This is an extension trait of [`Sipper`], for convenience.
pub trait Straw<Output, Progress = Output, Error = Never>:
    Sipper<Result<Output, Error>, Progress>
{
}

impl<S, Output, Progress, Error> Straw<Output, Progress, Error> for S where
    S: Sipper<Result<Output, Error>, Progress>
{
}
