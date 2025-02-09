use crate::{Future, Sink, Sipper};

use pin_project_lite::pin_project;

use std::marker::PhantomData;
use std::pin::Pin;
use std::task;

pin_project! {
    /// Runs the [`Sipper`], sending any progress through the given [`Sender`] and returning
    /// its output at the end.
    ///
    /// The result of [`Sipper::run`].
    pub struct Run<S, Si, Output, Progress>
    {
        #[pin]
        sipper: S,
        #[pin]
        on_progress: Si,
        state: State<Progress>,
        _types: PhantomData<(Output, Progress)>,
    }
}

impl<S, Si, Output, Progress> Run<S, Si, Output, Progress> {
    pub(crate) fn new(sipper: S, on_progress: Si) -> Self {
        Self {
            sipper,
            on_progress,
            state: State::Read,
            _types: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum State<T> {
    Read,
    Send(Option<T>),
    Flush,
    Output,
}

impl<S, Si, Output, Progress> Future for Run<S, Si, Output, Progress>
where
    S: Sipper<Output, Progress>,
    Si: Sink<Progress>,
{
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        use futures::ready;

        let mut this = self.project();

        loop {
            match this.state {
                State::Read => match ready!(this.sipper.as_mut().poll_next(cx)) {
                    Some(progress) => {
                        *this.state = State::Send(Some(progress));
                    }
                    None => {
                        *this.state = State::Output;
                    }
                },
                State::Send(progress) => match ready!(this.on_progress.as_mut().poll_ready(cx)) {
                    Ok(()) => {
                        let result = this
                            .on_progress
                            .as_mut()
                            .start_send(progress.take().unwrap());

                        if result.is_ok() {
                            *this.state = State::Flush;
                        } else {
                            *this.state = State::Output;
                        }
                    }
                    Err(_) => {
                        *this.state = State::Output;
                    }
                },
                State::Flush => match ready!(this.on_progress.as_mut().poll_flush(cx)) {
                    Ok(_) => {
                        *this.state = State::Read;
                    }
                    Err(_) => {
                        *this.state = State::Output;
                    }
                },
                State::Output => return this.sipper.poll(cx),
            }
        }
    }
}
