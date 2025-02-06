<div align="center">

# Sipper

[![Crates.io](https://img.shields.io/crates/v/sipper.svg)](https://crates.io/crates/sipper)
[![License](https://img.shields.io/crates/l/sipper.svg)](https://github.com/hecrj/sipper/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/sipper.svg)](https://crates.io/crates/sipper)
[![Test Status](https://img.shields.io/github/actions/workflow/status/hecrj/sipper/test.yml?branch=master&event=push&label=test)](https://github.com/hecrj/sipper/actions)

A sipper is a [`Future`] that can notify progress.
</div>

Effectively, a [`Sipper`] combines a [`Future`] and a [`Sink`]
together to represent an asynchronous task that produces some `Output`
and notifies of some `Progress`, without both types being necessarily the
same.

[`Sipper`] should be chosen over [`Stream`] when the final value produced—the
end of the task—is important and inherently different from the other values.

# An example
An example of this could be a file download. When downloading a file, the progress
that must be notified is normally a bunch of statistics related to the download; but
when the download finishes, the contents of the file need to also be provided.

## The Uncomfy Stream
With a [`Stream`], you must create some kind of type that unifies both states of the
download:

```rust
use futures::Stream;

struct File(Vec<u8>);

struct Progress(u32);

enum Download {
   Running(Progress),
   Done(File)
}

fn download(url: &str) -> impl Stream<Item = Download> {
    // ...
}
```

If we now wanted to notify progress and—at the same time—do something with
the final `File`, we'd need to juggle with the [`Stream`]:

```rust
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};

async fn example(mut on_progress: mpsc::Sender<Progress>) {
   let mut file_download = download("https://iced.rs/logo.svg").boxed();

   while let Some(download) = file_download.next().await {
       match download {
           Download::Running(progress) => {
               let _ = on_progress.send(progress).await;
           }
           Download::Done(file) => {
               // Do something with file...
               // We are nested, and there are no compiler guarantees
               // this will ever be reached. And how many times?
           }
       }
   }
}
```

While we could rewrite the previous snippet using `loop`, `expect`, and `break` to get the
final file out of the [`Stream`]. We would still be introducing runtime errors and, simply put,
working around the fact that a [`Stream`] does not encode the idea of a final value.

## The Chad Sipper
A [`Sipper`] can precisely describe this dichotomy in a type-safe way:

```rust
use sipper::Sipper;

#[derive(Debug, PartialEq, Eq)]
struct File(Vec<u8>);

#[derive(Debug, PartialEq, Eq)]
struct Progress(u32);

fn download(url: &str) -> impl Sipper<File, Progress> {
    // ...
}
```

Which can then be easily used with any [`Sink`]:

```rust
use futures::channel::mpsc;

async fn example(on_progress: mpsc::Sender<Progress>) {
    let file = download("https://iced.rs/logo.svg").run(on_progress).await;

    // We are guaranteed to have a `File` here!
}
```

[`Sipper`]: https://docs.rs/sipper/latest/sipper/trait.Sipper.html
[`Future`]: https://docs.rs/futures/0.3.31/futures/future/trait.Future.html
[`Sink`]: https://docs.rs/futures/0.3.31/futures/sink/trait.Sink.html
[`Stream`]: https://docs.rs/futures/0.3.31/futures/stream/trait.Stream.html
