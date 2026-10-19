use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::task::{self, JoinError};
use tracing::{debug, error};

use crate::error::{Error, Result};
use crate::instance::Cspcl;
use crate::{CspAddress, InboundStream};

impl Cspcl {
    /// Send a bundle to a remote CSP address
    ///
    /// This will automatically fragment the bundle if it exceeds CSP MTU
    /// and handle reassembly on the receiving end.
    ///
    /// # Arguments
    /// * `bundle` - The serialized bundle data to send
    /// * `dest` - Destination CSP address and port
    ///
    /// # Returns
    /// Ok(()) on success, or Err(Error) if the operation failed
    pub fn send_bundle(&self, bundle: &[u8], dest: CspAddress) -> Result<()> {
        self.send_bundle_unless_cancelled(bundle, dest, None)
    }

    fn send_bundle_unless_cancelled(
        &self,
        bundle: &[u8],
        dest: CspAddress,
        cancelled: Option<&AtomicBool>,
    ) -> Result<()> {
        if bundle.is_empty() {
            return Err(Error::InvalidParam);
        }

        debug!("Sending bundle to {}:{}", dest.addr, dest.port);
        match self.send_raw(bundle, dest, cancelled) {
            Ok(_) => debug!("Bundle sent to {}:{}", dest.addr, dest.port),
            Err(Error::Cancelled) => {
                // Nobody awaits this result.
                debug!(
                    "Send to {}:{} cancelled before it started",
                    dest.addr, dest.port
                );
                return Err(Error::Cancelled);
            }
            Err(e) => {
                error!(
                    "Could not send bundle to {}:{} : {}",
                    dest.addr,
                    dest.port,
                    e.to_string()
                );
                return Err(e);
            }
        };

        Ok(())
    }

    /// Send a bundle to a remote CSP address without blocking the async runtime
    ///
    /// [`send_bundle`](Self::send_bundle) blocks until the receiver acknowledges
    /// the bundle, which can take up to twice `CSPCL_ACK_TIMEOUT_MS` when the
    /// peer is unreachable. This variant runs it on tokio's blocking thread
    /// pool so that async callers do not stall a runtime worker.
    ///
    /// Sends are still serialized: the C connection pool is locked for the
    /// whole send/ack cycle.
    ///
    /// # Cancellation
    /// Dropping the returned future (e.g. on a timeout) before the send has
    /// started skips it, including while it is queued behind other sends. A
    /// send that has already started cannot be interrupted and runs to
    /// completion, so the bundle may still be delivered: treat a cancelled
    /// send as having an unknown outcome. The receiver must already
    /// tolerate duplicates, since a lost ack causes a resend (see the
    /// Delivery Acknowledgement docs).
    ///
    /// # Panics
    /// Propagates a panic from the blocking send task.
    pub async fn send_bundle_async(
        self: &Arc<Self>,
        bundle: impl Into<Vec<u8>>,
        dest: CspAddress,
    ) -> Result<()> {
        let bundle = bundle.into();
        let cspcl = Arc::clone(self);
        let cancelled = Arc::new(AtomicBool::new(false));
        let _cancel_on_drop = CancelOnDrop(Arc::clone(&cancelled));

        let send = task::spawn_blocking(move || {
            cspcl.send_bundle_unless_cancelled(&bundle, dest, Some(&cancelled))
        });

        flatten_join(send.await)
    }

    /// Open a stream of inbound bundles.
    ///
    /// Receiving runs on its own threads (one accept thread, one per inbound
    /// connection) while at least one stream is open
    ///
    /// # Streams
    /// - Bundles go to the most recently opened stream. Bundles left unread
    ///   in a dropped stream go to the newest other stream; there is no order
    ///   across streams.
    /// - Receiving stops when the last stream is dropped or
    ///   [`close_rx_socket`](Self::close_rx_socket) ends the streams. No new
    ///   read starts, but a read in progress (up to 10 s on an idle
    ///   connection) still acknowledges its bundle to the sender. Such
    ///   bundles, and those left unread, go to the next stream opened on this
    ///   instance; they are lost if the instance is dropped first.
    /// - Once receiving has been stopped for 1 s *and* its read in progress
    ///   has returned, a connection is closed, unless a stream was opened
    ///   meanwhile. So a stream reopened within 1 s keeps all connections.
    /// - Dropping the last handle to the instance does not stop reads in
    ///   progress: listeners keep the instance alive until their read
    ///   returns and the 1 s above has passed, and the accept thread until
    ///   its poll ends. As receiving stopped when the last stream closed,
    ///   that is up to about 10 s after it (the longer of a read and the
    ///   1 s), plus one accept poll; longer while a multi-fragment bundle is
    ///   in transfer, as the read timeout restarts for each fragment. The
    ///   bundles they acknowledge meanwhile are lost. A new
    ///   instance on the same port can be created meanwhile (the receive
    ///   socket is shared by the process), but the old connections still
    ///   use slots of libcsp's connection table, which is sized for a single
    ///   instance: until they are closed, the limit below no longer keeps
    ///   it from filling up, and further peers may be refused.
    ///
    /// # Connections
    /// - At most 8 stay open: a further peer is accepted at once and the
    ///   least recently active other connection closed once its read
    ///   returns; meanwhile up to 2 more are accepted, further ones wait in
    ///   libcsp's backlog. A peer reconnecting from the same address and
    ///   port replaces its old connection. libcsp has one receive socket per
    ///   process: receive through a single instance per process for this
    ///   limit to hold.
    /// - A connection reset by its peer is closed as soon as libcsp wakes its
    ///   read. Telling that wake-up from a timeout assumes the peer uses the
    ///   same RDP connection timeout as this node (libcsp's 10 s default,
    ///   which cspcl does not change). With a longer one, a reset late in a
    ///   read is only caught by the idle close; with a shorter one, idle
    ///   connections are closed after each read and the peer reconnects on
    ///   its next send.
    /// - A connection without a bundle for 10 min is closed, which also frees
    ///   one whose peer vanished without a reset.
    ///
    /// # Errors
    /// - If the accept thread cannot be started, the stream yields
    ///   [`Error::Spawn`] and ends, and so do streams opened while it was
    ///   being started. If it stops unexpectedly, the open streams end.
    /// - If a connection's thread cannot be started, the connection is
    ///   closed and the newest stream yields [`Error::Spawn`] but stays open;
    ///   accepting resumes after 1 s.
    ///
    /// # Blocking
    /// A call made while no accept thread runs (no stream open, or after it
    /// failed) starts one, and a call made while
    /// [`close_rx_socket`](Self::close_rx_socket) runs waits for it (see
    /// there for how long). From async code, call it with
    /// `tokio::task::spawn_blocking` where such a pause matters.
    pub fn inbound(self: Arc<Self>) -> InboundStream {
        InboundStream::new(self)
    }
}

/// Sets the flag when dropped, i.e. when the `send_bundle_async` future is
/// dropped before or after completion (setting it afterwards is harmless).
struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Unwrap the result of a blocking task, propagating its panic.
///
/// Blocking tasks are only cancelled when the runtime shuts down; that is
/// reported as [`Error::Cancelled`], not as a send failure.
fn flatten_join<T>(joined: std::result::Result<Result<T>, JoinError>) -> Result<T> {
    match joined {
        Ok(result) => result,
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => Err(Error::Cancelled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelled_task_maps_to_cancelled_error() {
        let task = tokio::spawn(std::future::pending::<Result<()>>());
        task.abort();
        let joined = task.await;
        assert!(joined.as_ref().is_err_and(JoinError::is_cancelled));

        assert_eq!(flatten_join(joined), Err(Error::Cancelled));
    }

    #[tokio::test]
    async fn task_result_is_passed_through() {
        let ok = tokio::spawn(async { Ok::<_, Error>(7) }).await;
        assert_eq!(flatten_join(ok), Ok(7));

        let err = tokio::spawn(async { Err::<(), _>(Error::Timeout) }).await;
        assert_eq!(flatten_join(err), Err(Error::Timeout));
    }

    #[tokio::test]
    #[should_panic(expected = "boom")]
    async fn task_panic_is_propagated() {
        let joined = tokio::spawn(async { panic!("boom") }).await;
        let _: Result<()> = flatten_join(joined);
    }
}
