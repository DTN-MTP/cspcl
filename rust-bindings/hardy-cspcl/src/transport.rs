use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use cspcl_bindings::{CspAddress, Error as CspclError, InboundStream};
use tokio::sync::oneshot;
use tracing::{debug, warn};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transport init failed: {0}")]
    Init(#[from] CspclError),
    #[error("transport send failed: {0}")]
    Send(#[source] CspclError),
    #[error("transport receive failed: {0}")]
    Recv(#[source] CspclError),
}

#[derive(Clone)]
pub struct Transport {
    cspcl: Arc<cspcl_bindings::Cspcl>,
    /// Set by [`cleanup`](Self::cleanup). Held while a stream is opened and
    /// while the receive socket is closed, so no stream is opened once
    /// cleanup started, and one being opened is ended by the close.
    closed: Arc<Mutex<bool>>,
}

impl Transport {
    pub fn new(cspcl: Arc<cspcl_bindings::Cspcl>) -> Self {
        Self {
            cspcl,
            closed: Arc::default(),
        }
    }

    pub async fn send_bundle(
        &self,
        payload: impl Into<Vec<u8>>,
        dest: CspAddress,
    ) -> Result<(), Error> {
        debug!("Try sending bundle to: {}:{}", dest.addr, dest.port);
        self.cspcl
            .send_bundle_async(payload, dest)
            .await
            .map_err(Error::Send)
    }

    /// Open an inbound stream, unless [`cleanup`](Self::cleanup) started.
    ///
    /// Opening may wait for a close of the receive socket in progress and
    /// starts a thread, so it runs on a thread of its own: not on tokio's
    /// blocking pool, where it could queue behind sends that each hold a
    /// thread until their peer acknowledges or times out.
    pub async fn inbound_stream(&self) -> Result<InboundStream, OpenError> {
        let (cspcl, closed) = (Arc::clone(&self.cspcl), Arc::clone(&self.closed));
        let (tx, rx) = oneshot::channel();
        std::thread::Builder::new()
            .name("hardy-cspcl-open".into())
            .spawn(move || {
                let closed = lock(&closed);
                let _ = tx.send((!*closed).then(|| cspcl.inbound()));
            })
            .map_err(|e| {
                warn!("Could not start a thread to open the inbound stream: {e}");
                OpenError::Failed
            })?;
        match rx.await {
            Ok(Some(stream)) => Ok(stream),
            Ok(None) => Err(OpenError::Closed),
            // The thread panicked.
            Err(_) => Err(OpenError::Failed),
        }
    }

    /// Stop receiving for good: end the open stream and open no other.
    ///
    /// Runs inline, so it also happens during runtime shutdown or outside a
    /// runtime. It blocks once, when the CLA is unregistered: first for an
    /// open in progress (which starts cspcl's accept thread), then for at
    /// most one accept poll (see `Cspcl::close_rx_socket`).
    pub fn cleanup(&self) {
        let mut closed = lock(&self.closed);
        *closed = true;
        self.cspcl.close_rx_socket();
    }
}

/// Lock a mutex whose contents stay consistent even if a holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Why [`Transport::inbound_stream`] opened no stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// [`Transport::cleanup`] started: receiving has stopped for good.
    Closed,
    /// No thread could be started to open it; a later attempt may succeed.
    Failed,
}
