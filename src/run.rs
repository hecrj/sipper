use crate::{Core, Future, Sink};

use pin_project_lite::pin_project;

use std::pin::Pin;
use std::task;

pin_project! {
    /// Runs the [`Sipper`], sending any progress through the given [`Sender`] and returning
    /// its output at the end.
    ///
    /// The result of [`Sipper::run`].
    pub struct Run<S: Core, Si>
    {
        #[pin]
        sipper: S,
        #[pin]
        on_progress: Si,
        state: State<S::Progress>,
    }
}

impl<S: Core, Si> Run<S, Si> {
    pub(crate) fn new(sipper: S, on_progress: Si) -> Self {
        Self {
            sipper,
            on_progress,
            state: State::Read,
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

impl<S, Si> Future for Run<S, Si>
where
    S: Core,
    Si: Sink<S::Progress>,
{
    type Output = <S as Core>::Output;

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
