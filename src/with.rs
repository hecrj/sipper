use crate::{Core, Future, Stream};

use pin_project_lite::pin_project;

use std::marker::PhantomData;
use std::pin::Pin;
use std::task;

pin_project! {
    /// Maps the progress of a [`Sipper`].
    ///
    /// The result of [`Sipper::with`].
    pub struct With<S, F, A>
    {
        #[pin]
        sipper: S,
        mapper: F,
        _types: PhantomData<A>,
    }
}

impl<S, F, A> With<S, F, A> {
    pub(crate) fn new(sipper: S, mapper: F) -> Self {
        Self {
            sipper,
            mapper,
            _types: PhantomData,
        }
    }
}

impl<S, F, A> Future for With<S, F, A>
where
    S: Core,
{
    type Output = <S as Core>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let this = self.project();
        this.sipper.poll(cx)
    }
}

impl<S, F, A> Stream for With<S, F, A>
where
    S: Core,
    F: FnMut(S::Progress) -> A,
{
    type Item = A;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        use futures::ready;

        let this = self.project();
        let mapped = ready!(this.sipper.poll_next(cx)).map(this.mapper);

        task::Poll::Ready(mapped)
    }
}
