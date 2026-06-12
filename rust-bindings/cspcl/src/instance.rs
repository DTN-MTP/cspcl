use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use cspcl_sys::types::AcceptedConn;

use crate::error::{Error, Result, from_sys_result};
use crate::interface::Interface;

/// Safe wrapper for CSPCL instance
#[derive(Clone)]
pub struct Cspcl {
    inner: Arc<RwLock<cspcl_sys::cspcl_t>>,
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

        Ok(Cspcl {
            inner: Arc::new(RwLock::new(cspcl)),
        })
    }

    /// Close the receive socket
    pub fn close_rx_socket(self) {
        let mut cspcl = self.inner_mut();
        cspcl_sys::types::close_rx_socket(&mut cspcl);
    }

    pub(crate) fn poll_connection(&self) -> Result<AcceptedConn> {
        let mut cspcl = self.inner_mut();
        let accepted_conn = from_sys_result(cspcl_sys::types::accept_conn(&mut cspcl, 10))?;
        from_sys_result(cspcl_sys::types::conn_pool_add_accepted(
            &mut cspcl,
            accepted_conn,
        ))?;
        Ok(accepted_conn)
    }

    /// Get local CSP address
    pub fn local_addr(&self) -> u8 {
        self.inner().local_addr
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.inner().initialized
    }

    /// Get mutable reference to inner CSPCL instance (for advanced usage)
    pub(crate) fn inner_mut(&self) -> RwLockWriteGuard<'_, cspcl_sys::cspcl_t> {
        self.inner.write().expect("CSPCL instance lock poisoned")
    }

    /// Get immutable reference to inner CSPCL instance (for advanced usage)
    pub(crate) fn inner(&self) -> RwLockReadGuard<'_, cspcl_sys::cspcl_t> {
        self.inner.read().expect("CSPCL instance lock poisoned")
    }
}

impl Drop for Cspcl {
    fn drop(&mut self) {
        let mut cspcl = self.inner_mut();
        cspcl_sys::types::cleanup(&mut cspcl);
    }
}
