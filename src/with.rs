use crate::{Future, Sipper, Stream};

use pin_project_lite::pin_project;

use std::marker::PhantomData;
use std::pin::Pin;
use std::task;

pin_project! {
    /// Maps the progress of a [`Sipper`].
    ///
    /// The result of [`Sipper::with`].
    pub struct With<S, Output, Progress, A, F>
    {
        #[pin]
        sipper: S,
        mapper: F,
        _types: PhantomData<(Output, Progress, A)>,
    }
}

impl<S, Output, Progress, A, F> With<S, Output, Progress, A, F> {
    pub(crate) fn new(sipper: S, mapper: F) -> Self {
        Self {
            sipper,
            mapper,
            _types: PhantomData,
        }
    }
}

impl<S, Output, Progress, A, F> Future for With<S, Output, Progress, A, F>
where
    S: Sipper<Output, Progress>,
{
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let this = self.project();
        this.sipper.poll(cx)
    }
}

impl<S, Output, Progress, A, F> Stream for With<S, Output, Progress, A, F>
where
    S: Sipper<Output, Progress>,
    F: FnMut(Progress) -> A,
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
