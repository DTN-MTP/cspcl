//! The CSP link: one per process.

use futures_util::Stream;
use log::{info, warn};
use std::sync::{
    Arc, Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{oneshot, watch};

use crate::{
    addr::{AddrError, CspclAddr},
    config::{Config, DEFAULT_CAN_IFACE, Interface},
    error::{SendError, check_size},
    rx::{self, OpenError as StreamOpenError, Received, Source},
};

/// Claimed by the [`CspclLink`] of this process. libcsp is a process-wide
/// singleton, and cspcl lets a second instance with the same address and
/// interface start: both would then receive from the same socket, splitting
/// the inbound bundles between them.
static CLAIMED: AtomicBool = AtomicBool::new(false);

/// Why [`CspclLink::new`] opened no link.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpenError {
    /// This process already opened a link (successfully). There is one per
    /// process, for its whole life.
    #[error("a cspcl link is already open in this process")]
    AlreadyOpen,
    /// The address or port is out of range.
    #[error("invalid cspcl config: {0}")]
    Config(#[from] AddrError),
    /// cspcl could not be started on this interface.
    #[error("could not start cspcl: {0}")]
    Init(#[source] cspcl::Error),
}

/// A cspcl link: sends bundles to CSP peers, and receives theirs.
///
/// There is one per process (see [`new`](Self::new)), shared by the tasks
/// that send and receive.
pub struct CspclLink {
    cspcl: Arc<cspcl::Cspcl>,
    /// Set by [`shutdown`](Self::shutdown). Held while a stream is opened and
    /// while the receive socket is closed, so no stream is opened once
    /// shutdown started, and one being opened is ended by the close.
    closed: Mutex<bool>,
    /// Set by [`shutdown`](Self::shutdown) before the receive socket is
    /// closed, without blocking readers; wakes the receive loop out of a
    /// backoff.
    shutdown: watch::Sender<bool>,
}

impl CspclLink {
    /// Start cspcl and open the link.
    ///
    /// Only the first successful call in a process opens a link: further
    /// ones return [`OpenError::AlreadyOpen`], even once it is dropped. A
    /// call that fails otherwise leaves the next one free to try again.
    pub fn new(config: &Config) -> Result<Arc<Self>, OpenError> {
        config.validate()?;
        if CLAIMED.swap(true, Ordering::AcqRel) {
            return Err(OpenError::AlreadyOpen);
        }
        let local = cspcl::CspAddress {
            addr: config.local_addr,
            port: config.port,
        };
        let cspcl = match cspcl::Cspcl::new(local, config.interface.clone().into()) {
            Ok(cspcl) => cspcl,
            Err(e) => {
                CLAIMED.store(false, Ordering::Release);
                return Err(OpenError::Init(e));
            }
        };

        info!(
            "cspcl link open as CSP {}:{} on {}",
            config.local_addr, config.port, config.interface
        );
        if config.interface == Interface::Can(None) {
            warn!(
                "cspcl uses the virtual CAN bus {DEFAULT_CAN_IFACE}: on real hardware, name the interface (e.g. can:can0)"
            );
        }

        Ok(Arc::new(Self {
            cspcl: Arc::new(cspcl),
            closed: Mutex::new(false),
            shutdown: watch::Sender::new(false),
        }))
    }

    /// Send a serialized bundle to `dest`, waiting until it acknowledges it.
    ///
    /// Sends are serialized: one to an unreachable peer holds the others
    /// back for up to about 20 s, until it times out. A bundle over
    /// [`MAX_BUNDLE_SIZE`](crate::MAX_BUNDLE_SIZE) is
    /// [`SendError::Permanent`] at once.
    ///
    /// Dropping the future before the send has started skips it; once
    /// started, it runs to completion, so the bundle may still be delivered.
    /// Sends keep working after [`shutdown`](Self::shutdown).
    pub async fn send(&self, bundle: impl Into<Vec<u8>>, dest: CspclAddr) -> Result<(), SendError> {
        let bundle = bundle.into();
        if check_size(bundle.len()).is_err() {
            return Err(SendError::Permanent(cspcl::Error::BundleTooLarge));
        }
        self.cspcl
            .send_bundle_async(bundle, dest.into())
            .await
            .map_err(SendError::from)
    }

    /// The bundles received from peers.
    ///
    /// Receiving starts when the stream is first polled: until then, no
    /// inbound connection is accepted, and peers' sends (including this
    /// node's sends to itself) time out.
    ///
    /// The stream only ends after [`shutdown`](Self::shutdown). If cspcl
    /// stops receiving otherwise, it reopens after a backoff (500 ms,
    /// doubling up to 30 s while it keeps failing quickly); errors on single
    /// connections are logged and skipped.
    ///
    /// Call it once, at startup, and keep the link for the life of the
    /// process: bundles go to the most recently opened stream only, and
    /// bundles acknowledged while none is open wait for the next one, but
    /// are lost if the link is dropped first.
    pub fn incoming(self: &Arc<Self>) -> impl Stream<Item = Received> + Send + Unpin + 'static {
        rx::receive(Arc::clone(self))
    }

    /// Stop receiving for good: end the [`incoming`](Self::incoming) streams,
    /// which are not reopened.
    ///
    /// It blocks the calling thread: for an open of the inbound stream in
    /// progress, then for at most one accept poll of cspcl (100 ms by
    /// default). From async code, call it with
    /// `tokio::task::spawn_blocking`, unless the process is exiting anyway
    /// and blocking a runtime worker that long does not matter.
    pub fn shutdown(&self) {
        let mut closed = lock(&self.closed);
        *closed = true;
        // Flagged before the close ends the streams, so the receive loop
        // takes the end for a shutdown, not a failure.
        self.shutdown.send_replace(true);
        self.cspcl.close_rx_socket();
    }
}

impl Source for Arc<CspclLink> {
    type Stream = cspcl::InboundStream;

    /// Opening may wait for a close of the receive socket in progress and
    /// starts a thread, so it runs on a thread of its own: not on tokio's
    /// blocking pool, where it could queue behind sends that each hold a
    /// thread until their peer acknowledges or times out.
    async fn open(&self) -> Result<Self::Stream, StreamOpenError> {
        let link = Arc::clone(self);
        let (tx, rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("picod3tn-cspcl-open".into())
            .spawn(move || {
                let closed = lock(&link.closed);
                let _ = tx.send((!*closed).then(|| Arc::clone(&link.cspcl).inbound()));
            })
            .map_err(|e| {
                warn!("Could not start a thread to open the cspcl inbound stream: {e}");
                StreamOpenError::Failed
            })?;
        match rx.await {
            Ok(Some(stream)) => Ok(stream),
            Ok(None) => Err(StreamOpenError::Closed),
            // The thread panicked.
            Err(_) => Err(StreamOpenError::Failed),
        }
    }

    fn is_closed(&self) -> bool {
        *self.shutdown.borrow()
    }

    async fn closed(&self) {
        // The sender lives as long as `self`: this only ends on shutdown.
        let _ = self.shutdown.subscribe().wait_for(|closed| *closed).await;
    }
}

impl From<CspclAddr> for cspcl::CspAddress {
    fn from(addr: CspclAddr) -> Self {
        Self {
            addr: addr.addr(),
            port: addr.port(),
        }
    }
}

impl From<Interface> for cspcl::Interface {
    fn from(interface: Interface) -> Self {
        match interface {
            Interface::Loopback => Self::Loopback,
            Interface::Can(iface) => Self::Can(iface.unwrap_or_else(|| DEFAULT_CAN_IFACE.into())),
            Interface::Zmqhub(host) => Self::Zmq(host),
        }
    }
}

/// Lock a mutex whose contents stay consistent even if a holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
