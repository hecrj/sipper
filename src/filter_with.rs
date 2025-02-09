use crate::{Core, Future, Stream};

use pin_project_lite::pin_project;

use std::marker::PhantomData;
use std::pin::Pin;
use std::task;

pin_project! {
    /// Maps and filters the progress of a [`Sipper`].
    ///
    /// The result of [`Sipper::filter_with`].
    pub struct FilterWith<S, F, A>
    {
        #[pin]
        sipper: S,
        mapper: F,
        _types: PhantomData<A>,
    }
}

impl<S, F, A> FilterWith<S, F, A> {
    pub(crate) fn new(sipper: S, mapper: F) -> Self {
        Self {
            sipper,
            mapper,
            _types: PhantomData,
        }
    }
}

impl<S, F, A> Future for FilterWith<S, F, A>
where
    S: Core,
{
    type Output = <S as Core>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let this = self.project();
        this.sipper.poll(cx)
    }
}

impl<S, F, A> Stream for FilterWith<S, F, A>
where
    S: Core,
    F: FnMut(S::Progress) -> Option<A>,
{
    type Item = A;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        use futures::ready;

        let mut this = self.project();

        let mapped = loop {
            match ready!(this.sipper.as_mut().poll_next(cx)).map(&mut this.mapper) {
                None => break None,
                Some(Some(value)) => break Some(value),
                Some(None) => {}
            }
        };

        task::Poll::Ready(mapped)
    }
}
