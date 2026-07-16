use std::sync::Arc;
use std::time::Duration;

use cspcl_bindings::{CspAddress, Error as CspclError, InboundStream};
use tracing::info;

use crate::bundle_debug::bundle_label;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transport init failed: {0}")]
    Init(#[from] CspclError),
    #[error("transport send failed: {0}")]
    Send(#[source] CspclError),
    #[error("transport receive failed: {0}")]
    Recv(#[source] CspclError),
    #[error("transport ping failed: {0}")]
    Ping(#[source] CspclError),
}

#[derive(Clone)]
pub struct Transport {
    cspcl: Arc<cspcl_bindings::Cspcl>,
}

impl Transport {
    pub fn new(cspcl: Arc<cspcl_bindings::Cspcl>) -> Self {
        Self { cspcl }
    }

    pub async fn send_bundle(
        &self,
        payload: impl Into<Vec<u8>>,
        dest: CspAddress,
    ) -> Result<(), Error> {
        let payload = payload.into();
        let label = bundle_label(&payload);
        info!(
            csp_addr = dest.addr,
            csp_port = dest.port,
            bundle_len = payload.len(),
            bundle = %label,
            "CSPCL outbound bundle selected for send"
        );
        self.cspcl.send_bundle(&payload, dest).map_err(Error::Send)
    }

    pub fn ping(&self, dest_addr: u8, timeout: Duration) -> Result<(), Error> {
        self.cspcl.ping(dest_addr, timeout).map_err(Error::Ping)
    }

    pub fn inbound_stream(&self) -> InboundStream {
        Arc::clone(&self.cspcl).inbound()
    }

    pub fn cleanup(&self) {
        self.cspcl.close_rx_socket();
    }
}
