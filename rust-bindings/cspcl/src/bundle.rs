use crate::cspcl_sys;
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
    ///
    /// # Returns
    /// Ok(()) on success, or Err(Error) if the operation failed
    pub fn send_bundle(&mut self, bundle: &[u8], dest_addr: u8) -> Result<()> {
        if bundle.is_empty() {
            return Err(Error::from_code(cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM).unwrap_err());
        }

        unsafe {
            Error::from_code(cspcl_sys::cspcl_send_bundle(
                self.inner_mut(),
                bundle.as_ptr(),
                bundle.len(),
                dest_addr,
            ))?;
        }
        Ok(())
    }

    /// Receive a bundle with optional timeout
    ///
    /// This function blocks until a complete bundle is received or timeout occurs.
    /// Incomplete bundles are automatically reassembled.
    ///
    /// # Arguments
    /// * `timeout_ms` - Timeout in milliseconds (0 = no timeout)
    ///
    /// # Returns
    /// Ok((bundle_data, source_address)) on success
    /// Err(Error) if the operation timed out or failed
    pub fn recv_bundle(&mut self, timeout_ms: u32) -> Result<(Vec<u8>, u8)> {
        // TODO: Consider making buffer size configurable via constructor or method parameter
        let mut buffer = vec![0u8; cspcl_sys::CSPCL_MAX_BUNDLE_SIZE as usize];
        let mut len = buffer.len();
        let mut src_addr: u8 = 0;

        unsafe {
            Error::from_code(cspcl_sys::cspcl_recv_bundle(
                self.inner_mut(),
                buffer.as_mut_ptr(),
                &mut len,
                &mut src_addr,
                timeout_ms,
            ))?;
        }

        buffer.truncate(len);
        Ok((buffer, src_addr))
    }
}
