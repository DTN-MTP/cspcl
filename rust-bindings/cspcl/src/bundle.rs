use std::sync::Arc;

use tracing::{debug, error};

use crate::InboundStream;
use crate::error::{Error, Result};
use crate::instance::Cspcl;

impl Cspcl {
    /// Send a bundle to a remote CSP address
    ///
    /// This will automatically fragment the bundle if it exceeds CSP MTU
    /// and handle reassembly on the receiving end.
    ///
    /// # Arguments
    /// * `bundle` - The serialized bundle data to send
    /// * `dest_addr` - Destination CSP address
    /// * `dest_port` - Destination CSP port
    ///
    /// # Returns
    /// Ok(()) on success, or Err(Error) if the operation failed
    pub fn send_bundle(&mut self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
        if bundle.is_empty() {
            return Err(Error::InvalidParam);
        }

        let mut inner = self.inner_mut();
        debug!("Sending bundle to {}:{}", dest_addr, dest_port);
        match cspcl_sys::types::send_bundle(&mut inner, bundle, dest_addr, dest_port)
            .map_err(Error::from_raw)
        {
            Ok(_) => debug!("Bundle sent to {}:{}", dest_addr, dest_port),
            Err(e) => error!(
                "Could not send bundle to {}:{} : {}",
                dest_addr,
                dest_port,
                e.to_string()
            ),
        };

        Ok(())
    }

    pub async fn inbound(self) -> InboundStream {
        let cspcl = Arc::new(self);
        InboundStream::new(cspcl.clone()).await
    }
}
