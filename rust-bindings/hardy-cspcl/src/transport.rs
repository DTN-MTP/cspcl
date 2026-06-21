use std::sync::Arc;

use cspcl_bindings::{Error as CspclError, InboundStream};
use hardy_async::sync::spin::RwLock;
use tracing::debug;

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
    cspcl: Arc<RwLock<cspcl_bindings::Cspcl>>,
}

impl Transport {
    pub fn new(cspcl: Arc<RwLock<cspcl_bindings::Cspcl>>) -> Self {
        Self { cspcl }
    }

    pub async fn send_bundle(
        &self,
        payload: impl Into<Vec<u8>>,
        addr: u8,
        port: u8,
    ) -> Result<(), Error> {
        debug!("Try sending bundle to: {}:{}", addr, port);
        self.cspcl
            .write()
            .send_bundle(&payload.into(), addr, port)
            .map_err(Error::Send)
    }

    pub async fn inbound_stream(&self) -> InboundStream {
        self.cspcl.read().clone().inbound().await
    }

    pub fn cleanup(&self) {
        let cspcl = self.cspcl.write();
        drop(cspcl);
    }
}
