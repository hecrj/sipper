//! Futures that notify progress.
use futures::channel::mpsc;
use futures::future::{BoxFuture, Either};
use futures::stream;

use std::marker::PhantomData;

pub use futures::never::Never;
pub use futures::{Future, FutureExt, Stream, StreamExt};

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
/// # enum Download {
/// #    Running(u32),
/// #    Done(File)
/// # }
/// #
/// # fn download(url: &str) -> impl Stream<Item = Download> {
/// #     // ...
/// #     futures::stream::once(async { Download::Done(File(Vec::new())) })
/// # }
/// use futures::{SinkExt, StreamExt};
///
/// async fn example() {
///    let mut file_download = download("https://iced.rs/logo.svg").boxed();
///
///    while let Some(download) = file_download.next().await {
///        match download {
///            Download::Running(progress) => {
///                println!("{progress}");
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
/// Which can then be easily used in a type-safe way:
///
/// ```rust
/// # use sipper::{sipper, Sipper};
/// #
/// # struct File(Vec<u8>);
/// #
/// # fn download(url: &str) -> impl Sipper<File, u32> {
/// #     sipper(|_| futures::future::ready(File(Vec::new())))
/// # }
/// #
/// async fn example() -> File {
///     let mut download = download("https://iced.rs/logo.svg").sip();
///
///     while let Some(progress) = download.next().await {
///         println!("{progress}");
///     }
///
///     let logo = download.finish().await;
///
///     // We are guaranteed to have a `File` here!
///     logo
/// }
/// ```
///
/// [`Stream`]: futures::Stream
/// [`Sink`]: futures::Sink
pub trait Sipper<Output, Progress = Output>: Sized {
    /// The future returned by this [`Sipper`].
    type Future: Future<Output = Output> + Send;

    /// Returns a [`Future`] that runs the [`Sipper`], sending any progress through the given [`Sender`].
    fn run_(self, on_progress: Sender<Progress>) -> Self::Future;

    /// Returns a [`Future`] that runs the [`Sipper`], sending any progress through the given [`Sender`].
    ///
    /// This is a generic version of [`run_`], for convenience.
    ///
    /// [`run_`]: Self::run_
    fn run(self, on_progress: impl Into<Sender<Progress>>) -> impl Future<Output = Output> + Send {
        self.run_(on_progress.into())
    }

    /// Returns a [`Sip`] that can be used to run the [`Sipper`] one step at a time.
    ///
    /// This is specially useful if you want to write a custom loop for handling the
    /// progress of the [`Sipper`]; while still being able to obtain the final value
    /// with compiler guarantees.
    fn sip<'a>(self) -> Sip<'a, Output, Progress>
    where
        Self::Future: 'a,
        Output: 'a,
        Progress: Send + 'a,
    {
        let (sender, receiver) = Sender::channel(1);
        let worker = self.run_(sender);

        let stream = stream::select(
            receiver.map(Either::Right),
            stream::once(worker).map(Either::Left),
        );

        Sip {
            stream: stream.boxed(),
            output: None,
            _progress: PhantomData,
        }
    }

    /// Transforms the progress of a [`Sipper`] with the given function; returning a new [`Sipper`].
    fn map<'a, T>(
        self,
        f: impl FnMut(Progress) -> T + Send + 'static,
    ) -> impl Sipper<Output, T, Future = BoxFuture<'a, Output>>
    where
        Self: Sized,
        Self::Future: 'a,
        Progress: Send + 'a,
        T: Send + 'a,
        Output: Send + 'a,
    {
        struct Map<'a, Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            F: FnMut(Progress) -> T + Send,
        {
            sipper: S,
            mapper: F,
            _types: PhantomData<(&'a Progress, O, T)>,
        }

        impl<'a, Progress, S, O, F, T> Sipper<O, T> for Map<'a, Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            S::Future: 'a,
            F: FnMut(Progress) -> T + Send + 'a,
            Progress: Send + 'a,
            T: Send + 'a,
            O: Send + 'a,
        {
            type Future = BoxFuture<'a, O>;

            fn run_(mut self, mut on_progress: Sender<T>) -> Self::Future {
                let mut sip = self.sipper.sip();

                async move {
                    while let Some(progress) = sip.next().await {
                        on_progress.send((self.mapper)(progress)).await;
                    }

                    sip.finish().await
                }
                .boxed()
            }
        }

        Map {
            sipper: self,
            mapper: f,
            _types: PhantomData,
        }
    }

    /// Transforms the progress of a [`Sipper`] with the given function; returning a new [`Sipper`].
    ///
    /// `None` values will be discarded and not notified.
    fn filter_map<'a, T>(
        self,
        f: impl FnMut(Progress) -> Option<T> + Send + 'static,
    ) -> impl Sipper<Output, T, Future = BoxFuture<'a, Output>>
    where
        Self: Sized,
        Self::Future: 'a,
        Progress: Send + 'a,
        T: Send + 'a,
        Output: Send + 'a,
    {
        struct FilterMap<'a, Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            F: FnMut(Progress) -> Option<T> + Send,
        {
            sipper: S,
            mapper: F,
            _types: PhantomData<(&'a Progress, O, T)>,
        }

        impl<'a, Progress, S, O, F, T> Sipper<O, T> for FilterMap<'a, Progress, S, O, F, T>
        where
            S: Sipper<O, Progress>,
            S::Future: 'a,
            F: FnMut(Progress) -> Option<T> + Send + 'a,
            Progress: Send + 'a,
            T: Send + 'a,
            O: Send + 'a,
        {
            type Future = BoxFuture<'a, O>;

            fn run_(mut self, mut on_progress: Sender<T>) -> Self::Future {
                let mut sip = self.sipper.sip();

                async move {
                    while let Some(progress) = sip.next().await {
                        if let Some(progress) = (self.mapper)(progress) {
                            on_progress.send(progress).await;
                        }
                    }

                    sip.finish().await
                }
                .boxed()
            }
        }

        FilterMap {
            sipper: self,
            mapper: f,
            _types: PhantomData,
        }
    }
}

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

/// A [`Sip`] lets you run a [`Sipper`] one step at a time.
///
/// Every [`next`] call produces some progress; while [`finish`]
/// can be called at any time to directly obtain the final output,
/// discarding any further progress notifications.
///
/// [`next`]: Self::next
/// [`finish`]: Self::finish
#[allow(missing_debug_implementations)]
pub struct Sip<'a, Output, Progress = Output> {
    stream: stream::BoxStream<'a, Either<Output, Progress>>,
    output: Option<Output>,
    _progress: PhantomData<Progress>,
}

impl<Output, Progress> Sip<'_, Output, Progress> {
    /// Gets the next progress, if any.
    ///
    /// When this method returns `None`, it means there
    /// is no more progress to be made; and the output is
    /// ready.
    pub async fn next(&mut self) -> Option<Progress> {
        if self.output.is_some() {
            return None;
        }

        while let Some(item) = self.stream.next().await {
            match item {
                Either::Left(output) => {
                    self.output = Some(output);
                }
                Either::Right(progress) => return Some(progress),
            }
        }

        None
    }

    /// Discards any further progress not obtained yet with [`next`] and
    /// obtains the final output.
    ///
    /// [`next`]: Self::next
    pub async fn finish(mut self) -> Output {
        if let Some(output) = self.output {
            return output;
        }

        // Discard all progress left
        while self.next().await.is_some() {}

        // We are guaranteed to have an output
        self.output.expect("A sipper must produce output!")
    }
}

/// A sender used to notify the progress of some [`Sipper`].
#[derive(Debug)]
pub struct Sender<T>(Sender_<T>);

#[derive(Debug)]
enum Sender_<T> {
    Null,
    Mpsc(mpsc::Sender<T>),
}

impl<T> Sender<T> {
    /// Creates a new [`Sender`] from an [`mpsc::Sender`].
    pub fn new(sender: mpsc::Sender<T>) -> Self {
        Self(Sender_::Mpsc(sender))
    }

    /// Creates a new channel with the given buffer capacity.
    pub fn channel(buffer: usize) -> (Self, mpsc::Receiver<T>) {
        let (sender, receiver) = mpsc::channel(buffer);

        (Self(Sender_::Mpsc(sender)), receiver)
    }

    /// Creates a new [`Sender`] that discards any progress.
    pub fn null() -> Self {
        Self(Sender_::Null)
    }

    /// Sends a value through the [`Sender`].
    ///
    /// Since we are only notifying progress, any channel errors
    /// are discarded.
    pub async fn send(&mut self, value: T) {
        use futures::SinkExt;

        if let Self(Sender_::Mpsc(raw)) = self {
            let _ = raw.send(value).await;
        }
    }
}

impl<T> From<mpsc::Sender<T>> for Sender<T> {
    fn from(sender: mpsc::Sender<T>) -> Self {
        Self(Sender_::Mpsc(sender))
    }
}

impl<T> From<&Sender<T>> for Sender<T> {
    fn from(sender: &Sender<T>) -> Self {
        sender.clone()
    }
}

impl<T> From<&mut Sender<T>> for Sender<T> {
    fn from(sender: &mut Sender<T>) -> Self {
        sender.clone()
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            Sender_::Null => Sender_::Null,
            Sender_::Mpsc(sender) => Sender_::Mpsc(sender.clone()),
        })
    }
}

/// A trait that can only be implemented by types with no values, like
/// [`Never`].
///
/// It is a useful trait to coerce error generics in a type safe way.
pub trait NeverError {
    /// A method that can never be called!
    fn never<T>(self) -> T;
}

impl NeverError for Never {
    fn never<T>(self) -> T {
        match self {}
    }
}

/// Creates a new [`Sipper`] from the given async closure, which receives
/// a [`Sender`] that can be used to notify progress asynchronously.
pub fn sipper<Progress, F>(
    builder: impl FnOnce(Sender<Progress>) -> F,
) -> impl Sipper<F::Output, Progress, Future = F>
where
    F: Future + Send,
{
    struct Internal<Progress, F, B>
    where
        F: Future,
        B: FnOnce(Sender<Progress>) -> F,
    {
        builder: B,
        _types: PhantomData<(Progress, F)>,
    }

    impl<Progress, F, B> Sipper<F::Output, Progress> for Internal<Progress, F, B>
    where
        F: Future + Send,
        B: FnOnce(Sender<Progress>) -> F,
    {
        type Future = F;

        fn run_(self, on_progress: Sender<Progress>) -> Self::Future {
            (self.builder)(on_progress)
        }
    }

    Internal {
        builder,
        _types: PhantomData,
    }
}

/// Turns the [`Sipper`] into a [`Stream`].
///
/// This is only possible if the `Output` and `Progress` types match!
pub fn stream<Output>(sipper: impl Sipper<Output>) -> impl Stream<Item = Output> + Send
where
    Output: Send,
{
    let (sender, receiver) = Sender::channel(1);

    stream::select(receiver, stream::once(sipper.run_(sender)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::channel::mpsc;
    use futures::StreamExt;

    use tokio::task;
    use tokio::test;

    type Progress = u32;

    #[derive(Debug, PartialEq, Eq)]
    struct File(Vec<u8>);

    #[derive(Debug, PartialEq, Eq)]
    enum Error {
        Failed,
    }

    fn download(url: &str) -> impl Sipper<File, Progress> + '_ {
        sipper(move |mut sender| async move {
            let _url = url;

            for i in 0..=100 {
                sender.send(i).await;
            }

            File(vec![1, 2, 3, 4])
        })
    }

    fn try_download(url: &str) -> impl Straw<File, Progress, Error> + '_ {
        sipper(move |mut sender| async move {
            let _url = url;

            for i in 0..=42 {
                sender.send(i).await;
            }

            Err(Error::Failed)
        })
    }

    #[test]
    async fn it_works() {
        let (sender, receiver) = mpsc::channel(1);

        let progress = task::spawn(receiver.collect::<Vec<_>>());
        let file = download("https://iced.rs/logo.svg").run(sender).await;

        assert!(progress
            .await
            .expect("Collect progress")
            .into_iter()
            .eq(0..=100));

        assert_eq!(file, File(vec![1, 2, 3, 4]));
    }

    #[test]
    async fn it_sips() {
        let mut i = 0;
        let mut last_progress = None;

        let mut download = download("https://iced.rs/logo.svg").sip();

        while let Some(progress) = download.next().await {
            i += 1;
            last_progress = Some(progress);
        }

        let file = download.finish().await;

        assert_eq!(i, 101);
        assert_eq!(last_progress, Some(100));
        assert_eq!(file, File(vec![1, 2, 3, 4]));
    }

    #[test]
    async fn it_sips_partially() {
        let mut download = download("https://iced.rs/logo.svg").sip();

        assert_eq!(download.next().await, Some(0));
        assert_eq!(download.next().await, Some(1));
        assert_eq!(download.next().await, Some(2));
        assert_eq!(download.next().await, Some(3));
        assert_eq!(download.finish().await, File(vec![1, 2, 3, 4]));
    }

    #[test]
    async fn it_can_be_streamed() {
        fn uses_stream<T>(_stream: impl Stream<Item = T> + Send) {
            // Do nothing
        }

        uses_stream(stream(
            download("https://iced.rs/logo.svg").map(|_| File(vec![])),
        ));
    }

    #[test]
    async fn it_can_fail() {
        let mut i = 0;
        let mut last_progress = None;

        let mut download = try_download("https://iced.rs/logo.svg").sip();

        while let Some(progress) = download.next().await {
            i += 1;
            last_progress = Some(progress);
        }

        let file = download.finish().await;

        assert_eq!(i, 43);
        assert_eq!(last_progress, Some(42));
        assert_eq!(file, Err(Error::Failed));
    }

    #[test]
    async fn it_composes_nicely() {
        use futures::stream::FuturesOrdered;

        fn download_all<'a>(urls: &'a [&str]) -> impl Sipper<Vec<File>, (usize, Progress)> + 'a {
            sipper(move |progress| async move {
                let downloads =
                    FuturesOrdered::from_iter(urls.iter().enumerate().map(|(id, url)| {
                        download(url)
                            .map(move |progress| (id, progress))
                            .run(&progress)
                    }));

                downloads.collect().await
            })
        }

        let mut download =
            download_all(&["https://iced.rs/logo.svg", "https://iced.rs/logo.white.svg"]).sip();

        let mut i = 0;

        while let Some(_progress) = download.next().await {
            i += 1;
        }

        let files = download.finish().await;

        assert_eq!(i, 202);
        assert_eq!(files, vec![File(vec![1, 2, 3, 4]), File(vec![1, 2, 3, 4])])
    }
}
