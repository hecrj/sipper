//! A sipper is a type-safe [`Future`] that can [`Stream`] progress.
//!
//! Effectively, a [`Sipper`] combines a [`Future`] and a [`Stream`]
//! together to represent an asynchronous task that produces some `Output`
//! and notifies of some `Progress`, without both types being necessarily the
//! same.
//!
//! In fact, a [`Sipper`] implements both the [`Future`] and the [`Stream`] traits—which
//! gives you all the great combinators from [`FutureExt`] and [`StreamExt`] for free.
//!
//! Generally, [`Sipper`] should be chosen over [`Stream`] when the final value produced—the
//! end of the task—is important and inherently different from the other values.
//!
//! # An example
//! An example of this could be a file download. When downloading a file, the progress
//! that must be notified is normally a bunch of statistics related to the download; but
//! when the download finishes, the contents of the file need to also be provided.
//!
//! ## The Uncomfy Stream
//! With a [`Stream`], you must create some kind of type that unifies both states of the
//! download:
//!
//! ```rust
//! use futures::Stream;
//!
//! struct File(Vec<u8>);
//!
//! type Progress = u32;
//!
//! enum Download {
//!     Running(Progress),
//!     Done(File)
//! }
//!
//! fn download(url: &str) -> impl Stream<Item = Download> {
//!     // ...
//! #     futures::stream::once(async { Download::Done(File(Vec::new())) })
//! }
//! ```
//!
//! If we now wanted to notify progress and—at the same time—do something with
//! the final `File`, we'd need to juggle with the [`Stream`]:
//!
//! ```rust
//! # use futures::Stream;
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! # enum Download {
//! #    Running(Progress),
//! #    Done(File)
//! # }
//! #
//! # fn download(url: &str) -> impl Stream<Item = Download> {
//! #     // ...
//! #     futures::stream::once(async { Download::Done(File(Vec::new())) })
//! # }
//! use futures::{SinkExt, StreamExt};
//!
//! async fn example() {
//!     let mut file_download = download("https://iced.rs/logo.svg").boxed();
//!
//!     while let Some(download) = file_download.next().await {
//!         match download {
//!             Download::Running(progress) => {
//!                 println!("{progress}%");
//!             }
//!             Download::Done(file) => {
//!                 // Do something with file...
//!                 // We are nested, and there are no compiler guarantees
//!                 // this will ever be reached.
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! While we could rewrite the previous snippet using `loop`, `expect`, and `break` to get the
//! final file out of the [`Stream`], we would still be introducing runtime errors and, simply put,
//! working around the fact that a [`Stream`] does not encode the idea of a final value.
//!
//! ## The Chad Sipper
//! A [`Sipper`] can precisely describe this dichotomy in a type-safe way:
//!
//! ```rust
//! use sipper::Sipper;
//!
//! struct File(Vec<u8>);
//!
//! type Progress = u32;
//!
//! fn download(url: &str) -> impl Sipper<File, Progress> {
//!     // ...
//! #     sipper::sipper(|_| futures::future::ready(File(Vec::new())))
//! }
//! ```
//!
//! Which can then be easily ~~used~~ sipped:
//!
//! ```rust
//! # use sipper::{sipper, Sipper};
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! # fn download(url: &str) -> impl Sipper<File, Progress> {
//! #     sipper(|_| futures::future::ready(File(Vec::new())))
//! # }
//! #
//! async fn example() -> File {
//!     let mut download = download("https://iced.rs/logo.svg").pin();
//!
//!     // A sipper is a stream!
//!     // `Sipper::sip` is actually just an alias of `Stream::next`
//!     while let Some(progress) = download.sip().await {
//!         println!("{progress}%");
//!     }
//!
//!     // A sipper is also a future!
//!     let logo = download.await;
//!
//!     // We are guaranteed to have a File here!
//!     logo
//! }
//! ```
//!
//! ## The Delicate Straw
//! How about error handling? Fear not! A [`Straw`] is a [`Sipper`] that can fail. What would
//! our download example look like with an error sprinkled in?
//!
//! ```rust
//! # use sipper::{sipper, Sipper};
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! use sipper::Straw;
//!
//! enum Error {
//!     Failed,
//! }
//!
//! fn try_download(url: &str) -> impl Straw<File, Progress, Error> {
//!     // ...
//! #     sipper(|_| futures::future::ready(Ok(File(Vec::new()))))
//! }
//!
//! async fn example() -> Result<File, Error> {
//!     let mut download = try_download("https://iced.rs/logo.svg").pin();
//!
//!     while let Some(progress) = download.sip().await {
//!         println!("{progress}%");
//!     }
//!
//!     let logo = download.await?;
//!
//!     // We are guaranteed to have a File here!
//!     Ok(logo)
//! }
//! ```
//!
//! Pretty much the same! It's quite easy to add error handling to an existing [`Sipper`].
//! In fact, [`Straw`] is actually just an extension trait of a [`Sipper`] with a `Result` as output.
//! Therefore, all the [`Sipper`] methods are available for [`Straw`] as well. It's just nicer to write!
//!
//! ## The Great Builder
//! You can build a [`Sipper`] with the [`sipper`] function. It takes a closure that receives
//! a [`Sender`]—for sending progress updates—and must return a [`Future`] producing the output.
//!
//! ```rust,ignore
//! # use sipper::{sipper, Sipper};
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! fn download(url: &str) -> impl Sipper<File, Progress> + '_ {
//!     sipper(|mut sender| async move {
//!         // Perform async request here...
//!         let download = /* ... */;
//!
//!         while let Some(chunk) = download.chunk().await {
//!             // ...
//!             // Send updates when needed
//!             sender.send(/* ... */).await;
//!
//!         }
//!
//!         File(/* ... */)
//!     })
//! }
//! ```
//!
//! Furthermore, [`Sipper`] has no required methods and is just an extension trait of a
//! [`Future`] and [`Stream`] combo. This means you can come up with new ways to build a
//! [`Sipper`] by implementing the async traits on any of your types. Additionally,
//! any foreign type that implements both is already one.
//!
//! ## The Fancy Composition
//! A [`Sipper`] supports a bunch of methods for easy composition; like [`with`], [`filter_with`],
//! and [`run`].
//!
//! For instance, let's say we wanted to build a new function that downloads a bunch of files
//! instead of just one:
//!
//! ```rust
//! # use sipper::{sipper, Sipper};
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! # fn download(url: &str) -> impl Sipper<File, Progress> {
//! #     sipper(|_| futures::future::ready(File(Vec::new())))
//! # }
//! #
//! fn download_all<'a>(urls: &'a [&str]) -> impl Sipper<Vec<File>, (usize, Progress)> + 'a {
//!     sipper(move |sender| async move {
//!         let mut files = Vec::new();
//!
//!         for (id, url) in urls.iter().enumerate() {
//!             let file = download(url)
//!                 .with(move |progress| (id, progress))
//!                 .run(&sender)
//!                 .await;
//!
//!             files.push(file);
//!         }
//!
//!         files
//!     })
//! }
//! ```
//!
//! As you can see, we just leverage [`with`] to combine the download index with the progress
//! and call [`run`] to drive the [`Sipper`] to completion—notifying properly through the [`Sender`].
//!
//! Of course, this example will download files sequentially; but, since [`run`] returns a simple
//! [`Future`], a proper collection like [`FuturesOrdered`] could be used just as easily—if not
//! more! Take a look:
//!
//! ```rust
//! # use sipper::{sipper, Sipper};
//! #
//! # struct File(Vec<u8>);
//! #
//! # type Progress = u32;
//! #
//! # fn download(url: &str) -> impl Sipper<File, Progress> {
//! #     sipper(|_| futures::future::ready(File(Vec::new())))
//! # }
//! #
//! use futures::stream::{FuturesOrdered, StreamExt};
//!
//! fn download_all<'a>(urls: &'a [&str]) -> impl Sipper<Vec<File>, (usize, Progress)> + 'a {
//!     sipper(|sender| {
//!         urls.iter()
//!             .enumerate()
//!             .map(|(id, url)| {
//!                 download(url)
//!                     .with(move |progress| (id, progress))
//!                     .run(&sender)
//!             })
//!             .collect::<FuturesOrdered<_>>()
//!             .collect()
//!     })
//! }
//! ```
//!
//! [`Stream`]: futures::Stream
//! [`Sink`]: futures::Sink
//! [`FutureExt`]: futures::FutureExt
//! [`StreamExt`]: futures::StreamExt
//! [`FuturesOrdered`]: futures::stream::FuturesOrdered
//! [`with`]: Sipper::with
//! [`filter_with`]: Sipper::filter_with
//! [`run`]: Sipper::run
mod core;
mod filter_with;
mod run;
mod sender;
mod straw;
mod with;

pub use core::Core;
pub use filter_with::FilterWith;
pub use run::Run;
pub use sender::Sender;
pub use straw::Straw;
pub use with::With;

use futures::channel::mpsc;
use futures::stream;
use pin_project_lite::pin_project;

use std::pin::Pin;
use std::task;

#[doc(no_inline)]
pub use futures::never::Never;
#[doc(no_inline)]
pub use futures::{Future, FutureExt, Sink, Stream, StreamExt};

/// A sipper is both a [`Stream`] that produces a bunch of progress
/// and a [`Future`] that produces some final output.
pub trait Sipper<Output, Progress = Output>:
    core::Core<Output = Output, Progress = Progress>
{
    /// Maps the progress of the [`Sipper`] with the given closure.
    ///
    /// This is analogous to `map` in many other types; but we use `with`
    /// to avoid naming collisions with [`Future`] and [`Stream`].
    fn with<F, A>(self, f: F) -> With<Self, F, A>
    where
        Self: Sized,
        F: FnMut(Progress) -> A,
    {
        With::new(self, f)
    }

    /// Maps and filters the progress of the [`Sipper`] with the given closure.
    ///
    /// This is analogous to `filter_map` in many other types; but we use `filter_with`
    /// to avoid naming collisions with [`Future`] and [`Stream`].
    fn filter_with<F, A>(self, f: F) -> FilterWith<Self, F, A>
    where
        Self: Sized,
        F: FnMut(Progress) -> Option<A>,
    {
        FilterWith::new(self, f)
    }

    /// Returns the next progress, if any.
    ///
    /// When this method returns `None`, it means there is no more progress to be made;
    /// and the output is ready.
    fn sip(&mut self) -> stream::Next<'_, Self>
    where
        Self: Unpin,
    {
        StreamExt::next(self)
    }

    /// Runs the [`Sipper`], sending any progress through the given [`Sender`] and returning
    /// its output at the end.
    fn run<S>(self, on_progress: impl Into<Sender<Progress, S>>) -> Run<Self, S>
    where
        Self: Sized,
        S: Sink<Progress>,
    {
        Run::new(self, on_progress.into().sink)
    }

    /// Pins the [`Sipper`] in a [`Box`].
    ///
    /// You may need to call this method before being able to [`sip`](Self::sip).
    fn pin(self) -> Pin<Box<Self>>
    where
        Self: Sized,
    {
        Box::pin(self)
    }
}

impl<T, Output, Progress> Sipper<Output, Progress> for T where
    T: core::Core<Output = Output, Progress = Progress>
{
}

/// Creates a new [`Sipper`] from the given async closure, which receives
/// a [`Sender`] that can be used to notify progress asynchronously.
pub fn sipper<Progress, F>(
    builder: impl FnOnce(Sender<Progress>) -> F,
) -> impl Sipper<F::Output, Progress>
where
    F: Future,
{
    pin_project! {
        struct Internal<F, Progress>
        where
            F: Future,
        {
            #[pin]
            future: F,
            #[pin]
            receiver: mpsc::Receiver<Progress>,
            output: Option<F::Output>,
            is_progress_finished: bool,
        }
    }

    impl<F, Progress> Future for Internal<F, Progress>
    where
        F: Future,
    {
        type Output = F::Output;

        fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
            let mut this = self.project();

            if !*this.is_progress_finished {
                loop {
                    match this.receiver.as_mut().poll_next(cx) {
                        task::Poll::Ready(Some(_)) => {} // Discard
                        task::Poll::Ready(None) => {
                            *this.is_progress_finished = true;
                            break;
                        }
                        task::Poll::Pending => {
                            break;
                        }
                    }
                }
            }

            if let Some(output) = this.output.take() {
                task::Poll::Ready(output)
            } else {
                this.future.poll(cx)
            }
        }
    }

    impl<F, Progress> Stream for Internal<F, Progress>
    where
        F: Future,
    {
        type Item = Progress;

        fn poll_next(
            self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> task::Poll<Option<Self::Item>> {
            use futures::ready;

            let mut this = self.project();

            if !*this.is_progress_finished {
                match this.receiver.as_mut().poll_next(cx) {
                    task::Poll::Ready(None) => {
                        *this.is_progress_finished = true;
                    }
                    task::Poll::Ready(progress) => return task::Poll::Ready(progress),
                    task::Poll::Pending => {}
                }
            }

            if this.output.is_some() {
                return task::Poll::Ready(None);
            }

            *this.output = Some(ready!(this.future.poll(cx)));

            if *this.is_progress_finished {
                task::Poll::Ready(None)
            } else {
                task::Poll::Pending
            }
        }
    }

    let (sender, receiver) = Sender::channel(1);

    Internal {
        future: builder(sender),
        receiver,
        is_progress_finished: false,
        output: None,
    }
}

/// Turns a [`Sipper`] into a [`Stream`].
///
/// This is only possible if the `Output` and `Progress` types of the [`Sipper`] match!
pub fn stream<Output>(sipper: impl Sipper<Output>) -> impl Stream<Item = Output> {
    let sip = sipper.pin();

    stream::unfold(Some(sip), |mut sip| async move {
        if let Some(progress) = sip.as_mut()?.next().await {
            Some((progress, sip))
        } else {
            Some((sip.take()?.await, sip))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::task;
    use tokio::test;

    #[derive(Debug, PartialEq, Eq)]
    struct File(Vec<u8>);

    type Progress = u32;

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
    async fn it_is_a_future() {
        assert_eq!(
            download("https://iced.rs/logo.svg").await,
            File(vec![1, 2, 3, 4])
        );
    }

    #[test]
    async fn it_is_a_stream() {
        assert!(download("https://iced.rs/logo.svg")
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .eq(0..=100));
    }

    #[test]
    async fn it_works() {
        use futures::StreamExt;

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

        let mut download = download("https://iced.rs/logo.svg").pin();

        while let Some(progress) = download.sip().await {
            i += 1;
            last_progress = Some(progress);
        }

        let file = download.await;

        assert_eq!(i, 101);
        assert_eq!(last_progress, Some(100));
        assert_eq!(file, File(vec![1, 2, 3, 4]));
    }

    #[test]
    async fn it_sips_partially() {
        let mut download = download("https://iced.rs/logo.svg").pin();

        assert_eq!(download.next().await, Some(0));
        assert_eq!(download.next().await, Some(1));
        assert_eq!(download.next().await, Some(2));
        assert_eq!(download.next().await, Some(3));
        assert_eq!(download.await, File(vec![1, 2, 3, 4]));
    }

    #[test]
    async fn it_sips_fully_and_completes() {
        let mut finished = false;

        {
            let mut download = sipper(|sender| async {
                let _ = download("https://iced.rs/logo.svg").run(sender).await;

                tokio::task::yield_now().await;
                tokio::task::yield_now().await;
                tokio::task::yield_now().await;

                finished = true;
            })
            .pin();

            while download.next().await.is_some() {}
        }

        assert!(finished);
    }

    #[test]
    async fn it_can_be_streamed() {
        async fn uses_stream(stream: impl Stream<Item = File> + Send) {
            use futures::StreamExt;
            let files: Vec<_> = stream.collect().await;

            assert_eq!(files.len(), 102);
            assert_eq!(files.last(), Some(&File(vec![1, 2, 3, 4])));
        }

        uses_stream(stream(
            download("https://iced.rs/logo.svg").with(|_| File(vec![])),
        ))
        .await;
    }

    #[test]
    async fn it_can_fail() {
        let mut i = 0;
        let mut last_progress = None;

        let mut download = try_download("https://iced.rs/logo.svg").pin();

        while let Some(progress) = download.next().await {
            i += 1;
            last_progress = Some(progress);
        }

        let file = download.await;

        assert_eq!(i, 43);
        assert_eq!(last_progress, Some(42));
        assert_eq!(file, Err(Error::Failed));
    }

    #[test]
    async fn it_can_be_mapped() {
        let mapper = |progress| progress * 2;

        let download = download("https://iced.rs/logo.svg")
            .with(mapper)
            .collect::<Vec<_>>()
            .await;

        assert_eq!(
            download.into_iter().sum::<u32>(),
            (0..=100).map(mapper).sum()
        );
    }

    #[test]
    async fn it_can_be_filtered() {
        let filter = |progress| (progress % 2 == 0).then_some(progress);

        let download = download("https://iced.rs/logo.svg")
            .filter_with(filter)
            .collect::<Vec<_>>()
            .await;

        assert_eq!(
            download.into_iter().sum::<u32>(),
            (0..=100).filter_map(filter).sum()
        );
    }

    #[test]
    async fn it_composes_nicely() {
        use futures::stream::{FuturesOrdered, StreamExt};

        fn download_all<'a>(urls: &'a [&str]) -> impl Sipper<Vec<File>, (usize, Progress)> + 'a {
            sipper(|sender| {
                urls.iter()
                    .enumerate()
                    .map(|(id, url)| {
                        download(url)
                            .with(move |progress| (id, progress))
                            .run(&sender)
                    })
                    .collect::<FuturesOrdered<_>>()
                    .collect()
            })
        }

        let mut download =
            download_all(&["https://iced.rs/logo.svg", "https://iced.rs/logo.white.svg"]).pin();

        let mut i = 0;

        while let Some(_progress) = download.next().await {
            i += 1;
        }

        let files = download.await;

        assert_eq!(i, 202);
        assert_eq!(files, vec![File(vec![1, 2, 3, 4]), File(vec![1, 2, 3, 4])]);
    }
}
