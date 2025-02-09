use crate::Sink;

use futures::channel::mpsc;
use futures::sink;

use std::marker::PhantomData;

/// A sender used to notify the progress of some [`Sipper`].
///
/// [`Sipper`]: crate::Sipper
#[derive(Debug)]
pub struct Sender<T, S = mpsc::Sender<T>> {
    pub(crate) sink: S,
    _output: PhantomData<T>,
}

impl<T> Sender<T> {
    /// Creates a new channel with the given buffer capacity.
    pub fn channel(buffer: usize) -> (Self, mpsc::Receiver<T>) {
        let (sender, receiver) = mpsc::channel(buffer);

        (
            Self {
                sink: sender,
                _output: PhantomData,
            },
            receiver,
        )
    }
}

impl<T> Sender<T, sink::Drain<T>> {
    /// Creates a new [`Sender`] that discards any progress.
    pub fn drain() -> Self {
        Self {
            sink: sink::drain(),
            _output: PhantomData,
        }
    }
}

impl<T, S> Sender<T, S>
where
    S: Sink<T>,
{
    /// Creates a new [`Sender`] from an [`mpsc::Sender`].
    pub fn new(sender: S) -> Self {
        Self {
            sink: sender,
            _output: PhantomData,
        }
    }

    /// Sends a value through the [`Sender`].
    ///
    /// Since we are only notifying progress, any channel errors
    /// are discarded.
    pub async fn send(&mut self, value: T)
    where
        S: Unpin,
    {
        use futures::SinkExt;

        let _ = self.sink.send(value).await;
    }
}

impl<T> From<mpsc::Sender<T>> for Sender<T> {
    fn from(sender: mpsc::Sender<T>) -> Self {
        Sender::new(sender)
    }
}

impl<T> From<&mpsc::Sender<T>> for Sender<T> {
    fn from(sender: &mpsc::Sender<T>) -> Self {
        Sender::new(sender.clone())
    }
}

impl<T, S> From<&Sender<T, S>> for Sender<T, S>
where
    S: Sink<T> + Clone,
{
    fn from(sender: &Sender<T, S>) -> Self {
        Sender::new(sender.clone().sink)
    }
}

impl<T, S> Clone for Sender<T, S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            _output: PhantomData,
        }
    }
}
