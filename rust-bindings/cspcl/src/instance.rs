use crate::error::{Error, Result};
use crate::interface::Interface;

/// Safe wrapper for CSPCL instance
pub struct Cspcl {
    inner: cspcl_sys::cspcl_t,
}

impl Cspcl {
    /// Initialize a new CSPCL instance with local CSP address
    pub fn new(local_addr: u8, local_port: u8, interface: Interface) -> Result<Self> {
        let config = cspcl_sys::types::CspclConfig {
            local_addr,
            csp_port: local_port,
            interface,
        };

        let cspcl = cspcl_sys::types::init_from_config(&config).map_err(Error::from_raw)?;

        Ok(Cspcl { inner: cspcl })
    }

    /// Close the receive socket
    pub fn close_rx_socket(&mut self) {
        cspcl_sys::types::close_rx_socket(&mut self.inner);
    }

    /// Get local CSP address
    pub fn local_addr(&self) -> u8 {
        self.inner.local_addr
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.inner.initialized
    }

    // TODO: Add a method to get reassembly stats/status if needed
    // pub fn reassembly_status(&self) -> ReassemblyStatus { ... }

    /// Get mutable reference to inner CSPCL instance (for advanced usage)
    pub fn inner_mut(&mut self) -> &mut cspcl_sys::cspcl_t {
        &mut self.inner
    }

    /// Get immutable reference to inner CSPCL instance (for advanced usage)
    pub fn inner(&self) -> &cspcl_sys::cspcl_t {
        &self.inner
    }
}

impl Drop for Cspcl {
    fn drop(&mut self) {
        cspcl_sys::types::cleanup(&mut self.inner);
    }
}
