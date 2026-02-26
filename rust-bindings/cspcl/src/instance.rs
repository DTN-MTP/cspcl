use crate::cspcl_sys;
use crate::error::{Error, Result};
use crate::interface::Interface;

/// Safe wrapper for CSPCL instance
pub struct Cspcl {
    inner: cspcl_sys::cspcl_t,
}

impl Cspcl {
    /// Initialize a new CSPCL instance with local CSP address
    pub fn new(local_addr: u8, local_port: u8, interface: Interface) -> Result<Self> {
        let mut cspcl = cspcl_sys::cspcl_t {
            local_addr,
            csp_port: local_port,
            iface_type: interface.clone().into(),
            ..Default::default()
        };

        match interface {
            Interface::Zmq(interface_name) => cspcl.zmqhub_addr = interface_name.into(),
            Interface::Can(interface_name) => cspcl.can_iface = interface_name.into(),
            _ => {}
        };

        unsafe {
            Error::from_code(cspcl_sys::cspcl_init(&mut cspcl))?;
        }

        Ok(Cspcl { inner: cspcl })
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

unsafe impl Send for Cspcl {}
unsafe impl Sync for Cspcl {}
