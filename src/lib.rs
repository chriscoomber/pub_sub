extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate log;

use std::fmt;
use futures::{Future, Stream, Sink};
use futures::stream::BoxStream;
use futures::sync::mpsc::{channel, Receiver, Sender};
use tokio_core::reactor::Handle;

/// Feed that can be subscribed to.
pub trait Feed<T> {
    /// Subscribe to this feed. The returned `Receiver` will be called when any changes are
    /// published on the channel.
    fn subscribe(&mut self) -> Receiver<T>;

    /// Spawn a task for this feed, consuming it.
    fn spawn(self, handle: Handle);
}

/// Feed implemented with a `Stream`-based backend.
pub struct StreamFeed<T> {
    /// `Senders` - one for each of our subscribers' channels.
    subscribers: Vec<Sender<T>>,
    /// Source of information, which is distributed by the feed amongst the subscribers.
    source: BoxStream<T, ()>,
}

impl<T> StreamFeed<T> {
    pub fn new(source: BoxStream<T, ()>) -> Self {
        StreamFeed {
            subscribers: Vec::new(),
            source: source,
        }
    }
}

impl<T: fmt::Debug + Clone + 'static> Feed<T> for StreamFeed<T> {
    fn subscribe(&mut self) -> Receiver<T> {
        // Create new channel for this subscriber and store off.
        let (sender, receiver) = channel::<T>(1);
        self.subscribers.push(sender);

        receiver
    }

    fn spawn(self, handle: Handle) {
        // Add logic for what to do when the receiver is completed. This is a future that completes
        // when the source stream stops sending information.
        let subscribers = self.subscribers;
        let future = self.source.for_each(move |change| {
            debug!("Received change: {:?}", change);

            // Kick each target with the change
            let iter = subscribers.iter().map(|sender| {
                debug!("Sending change to subscriber: {:?}", sender);
                sender.clone().send(change.clone())
            });

            // Collect iterators into a single future - after doing error handling.
            // TODO: actual error handling
            futures::stream::futures_unordered(iter).collect().map(|_| ()).map_err(|_| ())
        });

        // Put on reactor. This reactor will drive the source `Stream`, which by the above logic
        // will drive the subscriber `Sender`s (i.e. the subscriber `Sink`s). It is someone else's
        // job to drive the source `Sink` and the subscriber `Stream`s!
        debug!("Putting on thread pool");
        handle.spawn(future);
    }
}