use crate::{Future, Sipper, Stream};

use pin_project_lite::pin_project;

use std::marker::PhantomData;
use std::pin::Pin;
use std::task;

pin_project! {
    /// Maps and filters the progress of a [`Sipper`].
    ///
    /// The result of [`Sipper::filter_with`].
    pub struct FilterWith<S, Output, Progress, A,F>
    {
        #[pin]
        sipper: S,
        mapper: F,
        _types: PhantomData<(Output, Progress, A)>,
    }
}

impl<S, Output, Progress, A, F> FilterWith<S, Output, Progress, A, F> {
    pub(crate) fn new(sipper: S, mapper: F) -> Self {
        Self {
            sipper,
            mapper,
            _types: PhantomData,
        }
    }
}

impl<S, Output, Progress, A, F> Future for FilterWith<S, Output, Progress, A, F>
where
    S: Sipper<Output, Progress>,
{
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let this = self.project();
        this.sipper.poll(cx)
    }
}

impl<S, Output, Progress, A, F> Stream for FilterWith<S, Output, Progress, A, F>
where
    S: Sipper<Output, Progress>,
    F: FnMut(Progress) -> Option<A>,
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
