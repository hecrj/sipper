use crate::{Future, Stream};

/// The core trait of a [`Sipper`].
///
/// A [`Sipper`] is just _any_ type that is both a [`Future`] and a [`Stream`].
///
/// [`Sipper`]: crate::Sipper
pub trait Core: Future + Stream {}

impl<T> Core for T where T: Future + Stream {}
