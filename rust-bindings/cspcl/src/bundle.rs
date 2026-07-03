use std::sync::Arc;

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

        let mut inner = self.inner_mut();
        debug!("Sending bundle to {}:{}", dest.addr, dest.port);
        match cspcl_sys::types::send_bundle(&mut inner, bundle, dest.addr, dest.port)
            .map_err(Error::from_raw)
        {
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

    pub fn inbound(self: Arc<Self>) -> InboundStream {
        InboundStream::new(self)
    }
}
