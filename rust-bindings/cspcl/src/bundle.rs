use std::sync::Arc;

use tokio::task;
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
        if bundle.is_empty() {
            return Err(Error::InvalidParam);
        }

        debug!("Sending bundle to {}:{}", dest.addr, dest.port);
        match self.send_raw(bundle, dest) {
            Ok(_) => debug!("Bundle sent to {}:{}", dest.addr, dest.port),
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
    /// # Panics
    /// Propagates a panic from the blocking send task.
    pub async fn send_bundle_async(
        self: &Arc<Self>,
        bundle: impl Into<Vec<u8>>,
        dest: CspAddress,
    ) -> Result<()> {
        let bundle = bundle.into();
        let cspcl = Arc::clone(self);
        match task::spawn_blocking(move || cspcl.send_bundle(&bundle, dest)).await {
            Ok(result) => result,
            Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
            // Blocking tasks are only cancelled when the runtime shuts down.
            Err(_) => Err(Error::CspSend),
        }
    }

    pub fn inbound(self: Arc<Self>) -> InboundStream {
        InboundStream::new(self)
    }
}
