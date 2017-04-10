extern crate pub_sub;
extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate log;
extern crate fern;
extern crate time;

use std::thread;
use futures::{Future, Stream, Sink};
use futures::sync::mpsc::channel;
use tokio_core::reactor::Core;

use pub_sub::Feed;

fn main() {
    // Logging
    let logger_config = fern::DispatchConfig {
        format: Box::new(|msg, level, location| {
            // This format just displays [{level}] {message}
            format!("{} [{:5}] {:15} - {} ({}:{})",
                    time::now()
                        .strftime("%Y-%m-%d %H:%M:%S")
                        .expect("Invalid data format!"),
                    level,
                    location.module_path(),
                    msg,
                    location.file(),
                    location.line())
        }),
        // Output to stdout and the log file in the temporary directory we made above to test
        output: vec![fern::OutputConfig::stdout()],
        // Only log messages Debug and above
        level: log::LogLevelFilter::Info,
    };
    if let Err(e) = fern::init_global_logger(logger_config, log::LogLevelFilter::Trace) {
        error!("Failed to initialize global logger: {}", e);
    }

    let mut core = Core::new().unwrap();

    // Create a source which I can manually write integers into.
    let (source_sender, source_receiver) = channel::<i32>(1);

    // Turn into a feed
    let mut integer_feed = pub_sub::StreamFeed::new(source_receiver.boxed());

    // Subscribe to the feed and spin up a thread listening to it
    let subscriber_receiver = integer_feed.subscribe();
    thread::spawn(move || {
        subscriber_receiver.for_each(|item| {
            info!("Subscriber 1 received item: {:?}", item);
            Ok(())
        }).wait().unwrap();
    });

    // ... and again
    let subscriber_receiver = integer_feed.subscribe();
    thread::spawn(move || {
        subscriber_receiver.for_each(|item| {
            info!("Subscriber 2 received item: {:?}", item);
            Ok(())
        }).wait().unwrap();
    });

    // Activate the feed
    integer_feed.spawn(core.handle());

    // Spin up a thread which sends values in
    thread::spawn(move || {
        let mut data = vec![5, 4, 3, 2, 1];

        let mut send_future = futures::finished(source_sender).boxed();
        while let Some(i) = data.pop() {
            send_future = send_future.and_then(move |tx| {
                std::thread::sleep(std::time::Duration::from_secs(1));
                tx.send(i).map(move |tx| {
                    info!("Source sent: {:?}", i);
                    tx
                })
            }).boxed()
        }
        send_future.wait().unwrap();
    });

    // Turn the reactor
    info!("Turning reactor");
    core.run(futures::empty::<(), ()>()).unwrap();
}
