//! Futures that notify progress.
use futures::future;
use futures::{Future, Sink, SinkExt};

use std::marker::PhantomData;

/// A sipper is a [`Future`] that can notify progress.
///
/// Effectively, a [`Sipper`] combines a [`Future`] and a [`Sink`]
/// together to represent an asynchronous task that produces some `Output`
/// and notifies of some `Progress`, without both types being necessarily the
/// same.
///
/// [`Sipper`] should be chosen over [`Stream`] when the final value produced—the
/// end of the task—is important and inherently different from the other values.
///
/// # An example
/// An example of this could be a file download. When downloading a file, the progress
/// that must be notified is normally a bunch of statistics related to the download; but
/// when the download finishes, the contents of the file need to also be provided.
///
/// ## The Uncomfy Stream
/// With a [`Stream`], you must create some kind of type that unifies both states of the
/// download:
///
/// ```rust
/// use futures::Stream;
///
/// struct File(Vec<u8>);
///
/// struct Progress(u32);
///
/// enum Download {
///    Running(Progress),
///    Done(File)
/// }
///
/// fn download(url: &str) -> impl Stream<Item = Download> {
///     // ...
/// #     futures::stream::once(async { Download::Done(File(Vec::new())) })
/// }
/// ```
///
/// If we now wanted to notify progress and—at the same time—do something with
/// the final `File`, we'd need to juggle with the [`Stream`]:
///
/// ```rust
/// # use futures::Stream;
/// #
/// # struct File(Vec<u8>);
/// #
/// # struct Progress(u32);
/// #
/// # enum Download {
/// #    Running(Progress),
/// #    Done(File)
/// # }
/// #
/// # fn download(url: &str) -> impl Stream<Item = Download> {
/// #     // ...
/// #     futures::stream::once(async { Download::Done(File(Vec::new())) })
/// # }
/// use futures::{SinkExt, StreamExt};
/// use futures::channel::mpsc;
///
/// async fn example(mut on_progress: mpsc::Sender<Progress>) {
///    let mut file_download = download("https://iced.rs/logo.svg").boxed();
///
///    while let Some(download) = file_download.next().await {
///        match download {
///            Download::Running(progress) => {
///                let _ = on_progress.send(progress).await;
///            }
///            Download::Done(file) => {
///                // Do something with file...
///                // We are nested, and there are no compiler guarantees
///                // this will ever be reached.
///            }
///        }
///    }
/// }
/// ```
///
/// While we could rewrite the previous snippet using `loop`, `expect`, and `break` to get the
/// final file out of the [`Stream`]. We would still be introducing runtime errors and, simply put,
/// working around the fact that a [`Stream`] does not encode the idea of a final value.
///
/// ## The Chad Sipper
/// A [`Sipper`] can precisely describe this dichotomy in a type-safe way:
///
/// ```rust
/// use sipper::Sipper;
///
/// struct File(Vec<u8>);
///
/// struct Progress(u32);
///
/// fn download(url: &str) -> impl Sipper<File, Progress> {
///     // ...
/// #     sipper::sipper(|_| futures::future::ready(File(Vec::new())))
/// }
/// ```
///
/// Which can then be easily used with any [`Sink`]:
///
/// ```rust
/// # use sipper::{sipper, Sipper};
/// #
/// # struct File(Vec<u8>);
/// #
/// # struct Progress(u32);
/// #
/// # fn download(url: &str) -> impl Sipper<File, Progress> {
/// #     sipper(|_| futures::future::ready(File(Vec::new())))
/// # }
/// #
/// use futures::channel::mpsc;
///
/// async fn example(on_progress: mpsc::Sender<Progress>) {
///     let file = download("https://iced.rs/logo.svg").run(on_progress).await;
///
///     // We are guaranteed to have a `File` here!
/// }
/// ```
///
/// [`Stream`]: futures::Stream
pub trait Sipper<Output, Progress = Output> {
    /// Returns a [`Future`] that runs the [`Sipper`], sending any progress through the given [`Sender`].
    fn run_(self, on_progress: Sender<Progress>) -> impl Future<Output = Output> + Send;

    /// Returns a [`Future`] that runs the [`Sipper`], sending any progress through the given [`Sender`].
    ///
    /// This is just a generic version of [`run_`], for convenience.
    ///
    /// [`run_`]: Self::run_
    fn run(self, on_progress: impl IntoSender<Progress>) -> impl Future<Output = Output> + Send
    where
        Self: Sized,
    {
        self.run_(on_progress.into_sender())
    }

    /// Transforms the progress of a [`Sipper`] with the given function; returning a new [`Sipper`].
    fn map<T>(self, f: impl Fn(Progress) -> T + Send + 'static) -> impl Sipper<Output, T>
    where
        Self: Sized,
        Progress: 'static,
        T: Send + 'static,
    {
        struct Map<Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            F: Fn(Progress) -> T + Send,
        {
            sipper: S,
            mapper: F,
            _types: PhantomData<(Progress, O, T)>,
        }

        impl<Progress, S, O, F, T> Sipper<O, T> for Map<Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            F: Fn(Progress) -> T + Send,
            Progress: 'static,
            T: Send + 'static,
            F: 'static,
        {
            fn run_(self, on_progress: Sender<T>) -> impl Future<Output = O> {
                let mapper = self.mapper;

                let on_progress = Sender {
                    sink: Box::new(
                        on_progress
                            .sink
                            .with(move |progress| future::ready(Ok(mapper(progress)))),
                    ),
                };

                self.sipper.run_(on_progress)
            }
        }

        Map {
            sipper: self,
            mapper: f,
            _types: PhantomData,
        }
    }
}

/// A sender used to notify the progress of some [`Sipper`].
#[allow(missing_debug_implementations)]
pub struct Sender<T> {
    sink: Box<dyn Sink<T, Error = ()> + Unpin + Send>,
}

impl<T> Sender<T> {
    /// Sends a value through the [`Sender`].
    ///
    /// Since we are only notifying progress, any channel errors
    /// are discarded.
    pub async fn send(&mut self, value: T) {
        let _ = self.sink.send(value).await;
    }
}

/// A trait used to turn a type into a [`Sender`].
///
/// It is automatically implemented for all types that implement [`Sink`].
pub trait IntoSender<T> {
    /// Turns the type into a [`Sender`].
    fn into_sender(self) -> Sender<T>;
}

impl<S, T> IntoSender<T> for S
where
    S: Sink<T> + Unpin + Send + 'static,
{
    fn into_sender(self) -> Sender<T> {
        Sender {
            sink: Box::new(self.sink_map_err(|_| ())),
        }
    }
}

/// Creates a new [`Sipper`] from the given async closure, which receives
/// a [`Sender`] that can be used to notify progress asynchronously.
pub fn sipper<Progress, F>(
    builder: impl Fn(Sender<Progress>) -> F,
) -> impl Sipper<F::Output, Progress>
where
    F: Future + Send,
{
    struct Internal<Progress, F, B>
    where
        F: Future,
        B: Fn(Sender<Progress>) -> F,
    {
        builder: B,
        _types: PhantomData<(Progress, F)>,
    }

    impl<Progress, F, B> Sipper<F::Output, Progress> for Internal<Progress, F, B>
    where
        F: Future + Send,
        B: Fn(Sender<Progress>) -> F,
    {
        fn run_(self, on_progress: Sender<Progress>) -> impl Future<Output = F::Output> + Send {
            (self.builder)(on_progress)
        }
    }

    Internal {
        builder,
        _types: PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::channel::mpsc;
    use futures::StreamExt;

    use tokio::task;
    use tokio::test;

    #[test]
    async fn it_works() {
        #[derive(Debug, PartialEq, Eq)]
        struct Progress(u32);

        #[derive(Debug, PartialEq, Eq)]
        struct File(Vec<u8>);

        fn download() -> impl Sipper<File, Progress> {
            sipper(|mut sender| async move {
                for i in 0..=100 {
                    sender.send(Progress(i)).await;
                }

                File(vec![1, 2, 3, 4])
            })
        }

        let (sender, receiver) = mpsc::channel(1);

        let progress = task::spawn(receiver.collect::<Vec<_>>());
        let download = download().run(sender).await;

        assert!(progress
            .await
            .expect("Collect progress")
            .into_iter()
            .eq((0..=100).map(Progress)));

        assert_eq!(download, File(vec![1, 2, 3, 4]));
    }
}
