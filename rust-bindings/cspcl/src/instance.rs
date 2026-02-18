use crate::cspcl_sys;
use crate::error::{Error, Result};

/// Safe wrapper for CSPCL instance
pub struct Cspcl {
    inner: cspcl_sys::cspcl_t,
}

impl Cspcl {
    /// Initialize a new CSPCL instance with local CSP address
    pub fn new(local_addr: u8) -> Result<Self> {
        let mut cspcl = cspcl_sys::cspcl_t {
            initialized: false,
            local_addr: 0,
            next_fragment_id: 0,
            rx_socket: std::ptr::null_mut(),
            reassembly: unsafe { std::mem::zeroed() },
        };

        unsafe {
            Error::from_code(cspcl_sys::cspcl_init(&mut cspcl, local_addr))?;
        }

        Ok(Cspcl { inner: cspcl })
    }

    /// Open receive socket for listening to incoming bundles
    pub fn open_rx_socket(&mut self) -> Result<()> {
        unsafe {
            Error::from_code(cspcl_sys::cspcl_open_rx_socket(&mut self.inner))?;
        }
        Ok(())
    }

    /// Close the receive socket
    pub fn close_rx_socket(&mut self) {
        unsafe {
            cspcl_sys::cspcl_close_rx_socket(&mut self.inner);
        }
    }

    /// Get local CSP address
    pub fn local_addr(&self) -> u8 {
        self.inner.local_addr
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.inner.initialized
    }

    /// Clean up expired reassembly contexts
    pub fn cleanup_expired(&mut self) {
        unsafe {
            cspcl_sys::cspcl_cleanup_expired(&mut self.inner);
        }
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
        unsafe {
            cspcl_sys::cspcl_cleanup(&mut self.inner);
        }
    }
}
