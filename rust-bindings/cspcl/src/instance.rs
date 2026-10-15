use std::cell::UnsafeCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

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
    /// The C instance. It is only ever accessed through raw pointers because
    /// the C library mutates it from several threads at once (sends lock the
    /// connection pool internally); never create a Rust reference to it.
    inner: Box<UnsafeCell<cspcl_sys::cspcl_t>>,
    /// Guards the C instance's lifecycle. Send and accept are thread-safe in
    /// C and take the read side, so a long send (waiting for the peer's ack)
    /// never blocks accepting inbound connections. Calls that tear state down
    /// take the write side.
    lifecycle: RwLock<()>,
    inbound_shutdown: CancellationToken,
}

// SAFETY: all access to `inner` goes through the C library, which is
// thread-safe for send/accept; teardown is serialized by `lifecycle`.
unsafe impl Send for Cspcl {}
unsafe impl Sync for Cspcl {}

impl Cspcl {
    /// Initialize a new CSPCL instance with local CSP address
    pub fn new(local: CspAddress, interface: super::Interface) -> Result<Self> {
        let config = cspcl_sys::types::CspclConfig {
            local_addr: local.addr,
            csp_port: local.port,
            interface,
        };

        let cspcl = cspcl_sys::types::init_from_config(&config).map_err(Error::from_raw)?;

        Ok(Self::from_raw_instance(cspcl))
    }

    fn from_raw_instance(cspcl: cspcl_sys::cspcl_t) -> Self {
        Cspcl {
            inner: Box::new(UnsafeCell::new(cspcl)),
            lifecycle: RwLock::new(()),
            inbound_shutdown: CancellationToken::new(),
        }
    }

    /// Close the receive socket
    pub fn close_rx_socket(&self) {
        self.inbound_shutdown.cancel();
        let _guard = self.lifecycle_write();
        // SAFETY: exclusive lifecycle access.
        cspcl_sys::types::close_rx_socket(unsafe { &mut *self.inner.get() });
    }

    /// Accept one inbound connection.
    ///
    /// The returned connection is owned by the caller, who must close it with
    /// [`cspcl_sys::types::close_conn`] once done.
    pub(crate) fn poll_connection(&self) -> Result<AcceptedConn> {
        let _guard = self.lifecycle_read();
        // SAFETY: the instance is alive and not being torn down (read guard);
        // `cspcl_accept_conn` is safe to call concurrently with sends.
        from_sys_result(unsafe { cspcl_sys::types::accept_conn_raw(self.inner.get(), 10) })
    }

    /// Send a serialized bundle, blocking until the peer acknowledges it.
    pub(crate) fn send_raw(&self, bundle: &[u8], dest: CspAddress) -> Result<()> {
        let _guard = self.lifecycle_read();
        // SAFETY: the instance is alive and not being torn down (read guard);
        // `cspcl_send_bundle` locks the C connection pool internally.
        unsafe { cspcl_sys::types::send_bundle_raw(self.inner.get(), bundle, dest.addr, dest.port) }
            .map_err(Error::from_raw)
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        let _guard = self.lifecycle_read();
        // SAFETY: `initialized` is only written under the write guard.
        unsafe { (*self.inner.get()).initialized }
    }

    fn lifecycle_read(&self) -> RwLockReadGuard<'_, ()> {
        self.lifecycle
            .read()
            .expect("CSPCL lifecycle lock poisoned")
    }

    fn lifecycle_write(&self) -> RwLockWriteGuard<'_, ()> {
        self.lifecycle
            .write()
            .expect("CSPCL lifecycle lock poisoned")
    }

    pub(crate) fn inbound_shutdown_token(&self) -> CancellationToken {
        self.inbound_shutdown.clone()
    }

    #[cfg(test)]
    fn from_config_without_init_for_test(config: cspcl_sys::types::CspclConfig) -> Self {
        Self::from_raw_instance(cspcl_sys::types::configure(&config))
    }
}

impl Drop for Cspcl {
    fn drop(&mut self) {
        self.inbound_shutdown.cancel();
        cspcl_sys::types::cleanup(self.inner.get_mut());
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
