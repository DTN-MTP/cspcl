use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use cspcl_sys::types::AcceptedConn;
use tokio_util::sync::CancellationToken;

use crate::{
    CspAddress,
    error::{Error, Result, from_sys_result},
};

/// Safe wrapper for CSPCL instance
///
/// `Cspcl` owns the underlying C resource and must not be cloned into another
/// Rust owner that would also run cleanup on drop.
///
/// ```compile_fail
/// use cspcl::Cspcl;
///
/// fn duplicate_cleanup_owner(cspcl: Cspcl) {
///     let _clone = cspcl.clone();
/// }
/// ```
pub struct Cspcl {
    inner: Arc<RwLock<cspcl_sys::cspcl_t>>,
    inbound_shutdown: CancellationToken,
}

impl Cspcl {
    /// Initialize a new CSPCL instance with local CSP address
    pub fn new(local: CspAddress, interface: super::Interface) -> Result<Self> {
        let config = cspcl_sys::types::CspclConfig {
            local_addr: local.addr,
            csp_port: local.port,
            interface,
        };

        let cspcl = cspcl_sys::types::init_from_config(&config).map_err(Error::from_raw)?;

        Ok(Cspcl {
            inner: Arc::new(RwLock::new(cspcl)),
            inbound_shutdown: CancellationToken::new(),
        })
    }

    /// Close the receive socket
    pub fn close_rx_socket(&self) {
        self.inbound_shutdown.cancel();
        let mut cspcl = self.inner_mut();
        cspcl_sys::types::close_rx_socket(&mut cspcl);
    }

    /// Probe reachability of a remote CSP node (transport-level liveness).
    ///
    /// Returns `Ok(())` if the node replied within `timeout`, `Err(Error::Timeout)`
    /// if not. Blocking — call from a blocking context (e.g. `spawn_blocking`).
    pub fn ping(&self, dest_addr: u8, timeout: std::time::Duration) -> Result<()> {
        let inner_guard = self.inner();
        let inner = (&*inner_guard as *const cspcl_sys::cspcl_t).cast_mut();
        drop(inner_guard);

        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        unsafe { cspcl_sys::types::ping_ptr(inner, dest_addr, timeout_ms) }.map_err(Error::from_raw)
    }

    pub(crate) fn poll_connection(&self) -> Result<AcceptedConn> {
        let mut cspcl = self.inner_mut();
        from_sys_result(cspcl_sys::types::accept_conn(&mut cspcl, 10))
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

    pub(crate) fn inbound_shutdown_token(&self) -> CancellationToken {
        self.inbound_shutdown.clone()
    }

    #[cfg(test)]
    fn from_config_without_init_for_test(config: cspcl_sys::types::CspclConfig) -> Self {
        Cspcl {
            inner: Arc::new(RwLock::new(cspcl_sys::types::configure(&config))),
            inbound_shutdown: CancellationToken::new(),
        }
    }
}

impl Drop for Cspcl {
    fn drop(&mut self) {
        self.inbound_shutdown.cancel();
        let mut cspcl = self.inner_mut();
        cspcl_sys::types::cleanup(&mut cspcl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_cancels_inbound_shutdown_token() {
        let cspcl = Cspcl::from_config_without_init_for_test(cspcl_sys::types::CspclConfig {
            local_addr: 4,
            csp_port: 10,
            interface: cspcl_sys::types::InterfaceConfig::Loopback,
        });
        let inbound_shutdown = cspcl.inbound_shutdown_token();

        assert!(!inbound_shutdown.is_cancelled());

        drop(cspcl);

        assert!(inbound_shutdown.is_cancelled());
    }
}
