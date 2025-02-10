<div align="center">

# Sipper

[![Documentation](https://docs.rs/sipper/badge.svg)](https://docs.rs/sipper)
[![Crates.io](https://img.shields.io/crates/v/sipper.svg)](https://crates.io/crates/sipper)
[![License](https://img.shields.io/crates/l/sipper.svg)](https://github.com/hecrj/sipper/blob/master/LICENSE)
[![Downloads](https://img.shields.io/crates/d/sipper.svg)](https://crates.io/crates/sipper)
[![Test Status](https://img.shields.io/github/actions/workflow/status/hecrj/sipper/test.yml?branch=master&event=push&label=test)](https://github.com/hecrj/sipper/actions)

A sipper is a type-safe [`Future`] that can [`Stream`] progress.
</div>

Effectively, a [`Sipper`] combines a [`Future`] and a [`Stream`]
together to represent an asynchronous task that produces some `Output`
and notifies of some `Progress`, without both types being necessarily the
same.

In fact, a [`Sipper`] implements both the [`Future`] and the [`Stream`] traits—which
gives you all the great combinators from [`FutureExt`] and [`StreamExt`] for free.

Generally, [`Sipper`] should be chosen over [`Stream`] when the final value produced—the
end of the task—is important and inherently different from the other values.

# An Example
An example of this could be a file download. When downloading a file, the progress
that must be notified is normally a bunch of statistics related to the download; but
when the download finishes, the contents of the file need to also be provided.

## The Uncomfy Stream
With a [`Stream`], you must create some kind of type that unifies both states of the
download:

```rust
use futures::Stream;

struct File(Vec<u8>);

type Progress = u32;

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
use futures::StreamExt;

async fn example() {
    let mut file_download = download("https://iced.rs/logo.svg").boxed();

    while let Some(download) = file_download.next().await {
        match download {
            Download::Running(progress) => {
                println!("{progress}%");
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
final file out of the [`Stream`], we would still be introducing runtime errors and, simply put,
working around the fact that a [`Stream`] does not encode the idea of a final value.

## The Chad Sipper
A [`Sipper`] can precisely describe this dichotomy in a type-safe way:

```rust
use sipper::Sipper;

struct File(Vec<u8>);

type Progress = u32;

fn download(url: &str) -> impl Sipper<File, Progress> {
    // ...
}
```

Which can then be easily ~~used~~ sipped:

```rust
async fn example() -> File {
    let mut download = download("https://iced.rs/logo.svg").pin();

    // A sipper is a stream!
    // `Sipper::sip` is actually just an alias of `Stream::next`
    while let Some(progress) = download.sip().await {
        println!("{progress}%");
    }

    // A sipper is also a future!
    let logo = download.await;

    // We are guaranteed to have a File here!
    logo
}
```

## The Delicate Straw
How about error handling? Fear not! A [`Straw`] is a [`Sipper`] that can fail. What would
our download example look like with an error sprinkled in?

```rust
enum Error {
    Failed,
}

fn try_download(url: &str) -> impl Straw<File, Progress, Error> {
    // ...
}

async fn example() -> Result<File, Error> {
    let mut download = try_download("https://iced.rs/logo.svg").pin();
 
    while let Some(progress) = download.sip().await {
        println!("{progress}%");
    }
 
    let logo = download.await?;
 
    // We are guaranteed to have a File here!
    Ok(logo)
}
```

Pretty much the same! It's quite easy to add error handling to an existing [`Sipper`].
In fact, [`Straw`] is actually just an extension trait of a [`Sipper`] with a `Result` as output.
Therefore, all the [`Sipper`] methods are available for [`Straw`] as well. It's just nicer to write!

## The Great Builder
You can build a [`Sipper`] with the [`sipper`] function. It takes a closure that receives
a [`Sender`]—for sending progress updates—and must return a [`Future`] producing the output.

```rust
fn download(url: &str) -> impl Sipper<File, Progress> + '_ {
    sipper(|mut sender| async move {
        // Perform async request here...
        let download = /* ... */;

        while let Some(chunk) = download.chunk().await {
            // ...
            // Send updates when needed
            sender.send(/* ... */).await;

        }

        File(/* ... */)
    })
}
```

Furthermore, [`Sipper`] has no required methods and is just an extension trait of a
[`Future`] and [`Stream`] combo. This means you can come up with new ways to build a
[`Sipper`] by implementing the async traits on any of your types. Additionally,
any foreign type that implements both is already one.

## The Fancy Composition
A [`Sipper`] supports a bunch of methods for easy composition; like [`with`], [`filter_with`],
and [`run`].

For instance, let's say we wanted to build a new function that downloads a bunch of files
instead of just one:

```rust
fn download_all<'a>(urls: &'a [&str]) -> impl Sipper<Vec<File>, (usize, Progress)> + 'a {
    sipper(move |sender| async move {
        let mut files = Vec::new();

        for (id, url) in urls.iter().enumerate() {
            let file = download(url)
                .with(move |progress| (id, progress))
                .run(&sender)
                .await;

            files.push(file);
        }

        files
    })
}
```

As you can see, we just leverage [`with`] to combine the download index with the progress
and call [`run`] to drive the [`Sipper`] to completion—notifying properly through the [`Sender`].

Of course, this example will download files sequentially; but, since [`run`] returns a simple
[`Future`], a proper collection like [`FuturesOrdered`] could be used just as easily—if not
more! Take a look:

```rust
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
```

[`Sipper`]: https://docs.rs/sipper/latest/sipper/trait.Sipper.html
[`Straw`]: https://docs.rs/sipper/latest/sipper/trait.Straw.html
[`Sender`]: https://docs.rs/sipper/latest/sipper/struct.Sender.html
[`Future`]: https://docs.rs/futures/0.3.31/futures/future/trait.Future.html
[`Stream`]: https://docs.rs/futures/0.3.31/futures/stream/trait.Stream.html
[`FutureExt`]: https://docs.rs/futures/0.3.31/futures/future/trait.FutureExt.html
[`StreamExt`]: https://docs.rs/futures/0.3.31/futures/stream/trait.StreamExt.html
[`Sink`]: https://docs.rs/futures/0.3.31/futures/sink/trait.Sink.html
[`FuturesOrdered`]: https://docs.rs/futures/0.3.31/futures/stream/struct.FuturesOrdered.html
[`sipper`]: https://docs.rs/sipper/latest/sipper/fn.sipper.html
[`with`]: https://docs.rs/sipper/latest/sipper/trait.Sipper.html#method.with
[`filter_with`]: https://docs.rs/sipper/latest/sipper/trait.Sipper.html#method.filter_with
[`run`]: https://docs.rs/sipper/latest/sipper/trait.Sipper.html#method.run
