//! The receive loop behind [`CspclLink::incoming`](crate::CspclLink::incoming).

use futures_util::{Stream, StreamExt, stream};
use log::{debug, warn};
use std::{future::Future, time::Duration};
use tokio::time::Instant;

/// First pause before reopening an inbound stream that ended, or could not
/// be opened, without the link being shut down. It doubles on each further
/// failure, up to [`MAX_REOPEN_DELAY`], and starts over once a stream stayed
/// open for longer than the current delay.
///
/// Reopening only happens if cspcl's accept thread could not start or
/// failed. cspcl may have closed the inbound connections by then; peers
/// reconnect on their next send.
const REOPEN_DELAY: Duration = Duration::from_millis(500);

const MAX_REOPEN_DELAY: Duration = Duration::from_secs(30);

/// A bundle received from a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// The serialized bundle.
    pub data: Vec<u8>,
    /// The sender's CSP node address.
    pub src_addr: u8,
    /// The sender's CSP source port: an ephemeral port, not the one it
    /// listens on. Never send to it, nor use it to look the peer up.
    pub src_port: u8,
}

/// Why [`Source::open`] opened no stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenError {
    /// The link was shut down: receiving has stopped for good.
    Closed,
    /// No thread could be started to open it; a later attempt may succeed.
    Failed,
}

/// Where the receive loop gets its inbound streams from: the link, or a fake
/// in tests.
pub(crate) trait Source: Send + Sync + 'static {
    type Stream: Stream<Item = cspcl::Result<cspcl::Bundle>> + Send + Unpin + 'static;

    /// Open a stream, unless the link was shut down.
    fn open(&self) -> impl Future<Output = Result<Self::Stream, OpenError>> + Send;

    /// Whether the link was shut down. Set before the streams are ended.
    fn is_closed(&self) -> bool;

    /// Resolves once the link is shut down.
    fn closed(&self) -> impl Future<Output = ()> + Send;
}

/// Receive from `source` until it is shut down, reopening its stream when it
/// ends otherwise.
pub(crate) fn receive<S: Source>(source: S) -> impl Stream<Item = Received> + Send + Unpin {
    let state = State {
        source,
        stream: None,
        opened_at: Instant::now(),
        backoff: Backoff::new(),
    };
    Box::pin(
        stream::unfold(state, |mut state| async move {
            let received = state.next().await?;
            Some((received, state))
        })
        .fuse(),
    )
}

struct State<S: Source> {
    source: S,
    stream: Option<S::Stream>,
    opened_at: Instant,
    backoff: Backoff,
}

impl<S: Source> State<S> {
    async fn next(&mut self) -> Option<Received> {
        loop {
            let stream = match &mut self.stream {
                Some(stream) => stream,
                None => match self.source.open().await {
                    Ok(stream) => {
                        self.opened_at = Instant::now();
                        self.stream.insert(stream)
                    }
                    Err(OpenError::Closed) => return None,
                    Err(OpenError::Failed) => {
                        warn!(
                            "Could not open the cspcl inbound stream, retrying in {:?}",
                            self.backoff.delay
                        );
                        if !self.backoff.wait(&self.source).await {
                            return None;
                        }
                        continue;
                    }
                },
            };

            match stream.next().await {
                Some(Ok(bundle)) => {
                    debug!(
                        "Received a {} byte bundle from {}:{}",
                        bundle.data.len(),
                        bundle.src_addr,
                        bundle.src_port
                    );
                    return Some(Received {
                        data: bundle.data,
                        src_addr: bundle.src_addr,
                        src_port: bundle.src_port,
                    });
                }
                // A connection's thread could not start: cspcl closed that
                // connection, its peer reconnects on its next send.
                Some(Err(e)) => warn!("cspcl inbound error: {e}"),
                None => {
                    self.stream = None;
                    // Shutdown is flagged before it ends the streams.
                    if self.source.is_closed() {
                        return None;
                    }
                    // Otherwise cspcl's accept thread could not start or
                    // failed: without a stream, nothing would be received.
                    if self.opened_at.elapsed() > self.backoff.delay {
                        // Not a quick failure: earlier ones are over.
                        self.backoff = Backoff::new();
                    }
                    warn!(
                        "cspcl inbound stream ended, reopening it in {:?}",
                        self.backoff.delay
                    );
                    if !self.backoff.wait(&self.source).await {
                        return None;
                    }
                }
            }
        }
    }
}

/// The pause before the next attempt to open an inbound stream.
///
/// Copied from hardy-cspcl's `Backoff` (`hardy-cspcl/src/dispatcher.rs`),
/// waiting on the link's shutdown instead of a cancellation token: carry a
/// fix to one over to the other.
struct Backoff {
    delay: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            delay: REOPEN_DELAY,
        }
    }

    /// Wait the current delay, then double it for the next failure.
    /// Returns `false` if shut down first.
    async fn wait(&mut self, source: &impl Source) -> bool {
        let delay = self.delay;
        self.delay = (delay * 2).min(MAX_REOPEN_DELAY);
        tokio::select! {
            biased;
            _ = source.closed() => false,
            _ = tokio::time::sleep(delay) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream::BoxStream;
    use std::{
        collections::VecDeque,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
    };
    use tokio::sync::watch;

    /// What the next [`Fake::open`] does.
    enum Open {
        /// Fails to open.
        Fail,
        /// Yields these items, then ends.
        Items(Vec<cspcl::Result<cspcl::Bundle>>),
        /// Ends after this long.
        EndAfter(Duration),
    }

    #[derive(Clone)]
    struct Fake(Arc<FakeInner>);

    struct FakeInner {
        script: Mutex<VecDeque<Open>>,
        opened: Mutex<Vec<Instant>>,
        closed: AtomicBool,
        shutdown: watch::Sender<bool>,
    }

    impl Fake {
        fn new(script: impl IntoIterator<Item = Open>) -> Self {
            Self(Arc::new(FakeInner {
                script: Mutex::new(script.into_iter().collect()),
                opened: Mutex::default(),
                closed: AtomicBool::new(false),
                shutdown: watch::Sender::new(false),
            }))
        }

        fn shutdown(&self) {
            self.0.closed.store(true, Ordering::SeqCst);
            self.0.shutdown.send_replace(true);
        }

        /// The pauses between successive opens.
        fn gaps(&self) -> Vec<Duration> {
            let opened = self.0.opened.lock().unwrap();
            opened.windows(2).map(|w| w[1] - w[0]).collect()
        }

        fn opens(&self) -> usize {
            self.0.opened.lock().unwrap().len()
        }
    }

    impl Source for Fake {
        type Stream = BoxStream<'static, cspcl::Result<cspcl::Bundle>>;

        async fn open(&self) -> Result<Self::Stream, OpenError> {
            if self.is_closed() {
                return Err(OpenError::Closed);
            }
            self.0.opened.lock().unwrap().push(Instant::now());
            match self.0.script.lock().unwrap().pop_front() {
                Some(Open::Fail) => Err(OpenError::Failed),
                Some(Open::Items(items)) => Ok(stream::iter(items).boxed()),
                Some(Open::EndAfter(after)) => Ok(stream::once(tokio::time::sleep(after))
                    .filter_map(|()| async { None })
                    .boxed()),
                // Out of script: stay open, empty.
                None => Ok(stream::pending().boxed()),
            }
        }

        fn is_closed(&self) -> bool {
            self.0.closed.load(Ordering::SeqCst)
        }

        async fn closed(&self) {
            let _ = self.0.shutdown.subscribe().wait_for(|closed| *closed).await;
        }
    }

    fn bundle(data: &[u8]) -> cspcl::Result<cspcl::Bundle> {
        Ok(cspcl::Bundle {
            data: data.to_vec(),
            src_addr: 2,
            src_port: 40,
        })
    }

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    #[tokio::test(start_paused = true)]
    async fn yields_bundles_and_skips_errors() {
        let fake = Fake::new([Open::Items(vec![
            bundle(b"one"),
            Err(cspcl::Error::Spawn),
            bundle(b"two"),
        ])]);
        let mut incoming = receive(fake.clone());

        let first = incoming.next().await.unwrap();
        assert_eq!(
            first,
            Received {
                data: b"one".to_vec(),
                src_addr: 2,
                src_port: 40
            }
        );
        assert_eq!(incoming.next().await.unwrap().data, b"two");
        assert_eq!(fake.opens(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn backoff_doubles_up_to_its_cap() {
        let mut script: Vec<_> = (0..9).map(|_| Open::Fail).collect();
        script.push(Open::Items(vec![bundle(b"late")]));
        let fake = Fake::new(script);

        assert_eq!(receive(fake.clone()).next().await.unwrap().data, b"late");
        let expected: Vec<_> = [0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 30.0, 30.0, 30.0]
            .map(secs)
            .to_vec();
        assert_eq!(fake.gaps(), expected);
    }

    #[tokio::test(start_paused = true)]
    async fn ended_streams_are_reopened_and_long_lived_ones_reset_the_backoff() {
        let fake = Fake::new([
            Open::Items(vec![]),
            Open::Items(vec![]),
            Open::Items(vec![]),
            // Outlives the 4 s delay then due.
            Open::EndAfter(secs(10.0)),
            Open::Items(vec![]),
            Open::Items(vec![bundle(b"back")]),
        ]);

        assert_eq!(receive(fake.clone()).next().await.unwrap().data, b"back");
        let expected: Vec<_> = [0.5, 1.0, 2.0, 10.5, 1.0].map(secs).to_vec();
        assert_eq!(fake.gaps(), expected);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_interrupts_the_backoff() {
        let fake = Fake::new([Open::Fail]);
        let mut incoming = receive(fake.clone());

        let shutdown = tokio::spawn({
            let fake = fake.clone();
            async move {
                tokio::time::sleep(secs(0.1)).await;
                fake.shutdown();
            }
        });
        let start = Instant::now();
        assert_eq!(incoming.next().await, None);
        assert_eq!(start.elapsed(), secs(0.1));
        assert_eq!(incoming.next().await, None);
        assert_eq!(fake.opens(), 1);
        shutdown.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stream_ended_by_shutdown_is_not_reopened() {
        let fake = Fake::new([Open::EndAfter(secs(1.0))]);
        let mut incoming = receive(fake.clone());

        // Flagged while the stream is open, before it ends, as the link
        // does (the stream ending stands for the socket close).
        let flag = tokio::spawn({
            let fake = fake.clone();
            async move {
                tokio::time::sleep(secs(0.5)).await;
                fake.0.closed.store(true, Ordering::SeqCst);
            }
        });
        assert_eq!(incoming.next().await, None);
        assert_eq!(fake.opens(), 1);
        flag.await.unwrap();
    }
}
