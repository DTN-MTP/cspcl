use std::cell::UnsafeCell;
use std::sync::{
    Mutex, MutexGuard, PoisonError,
    atomic::{AtomicBool, Ordering},
};

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
    /// Guards `rx_socket`, the only field written after initialization:
    /// `cspcl_accept_conn` reopens the receive socket when it is closed, and
    /// [`close_rx_socket`](Self::close_rx_socket) clears it. Held for at most
    /// one short accept poll, never during a send.
    accept: Mutex<()>,
    /// Taken by every send before it enters C. `cspcl_send_bundle` already
    /// serializes all sends of an instance on the C pool mutex, so this adds
    /// no serialization; it moves the wait to Rust, where a send that was
    /// cancelled while queued can be skipped before it starts.
    send: Mutex<()>,
    inbound_shutdown: CancellationToken,
}

// SAFETY: all access to `inner` goes through the C library or raw pointers.
// Initialization completes before a `Cspcl` exists and cleanup only runs in
// `Drop`, when no other reference is left. In between, sends only touch the
// connection pool, which has its own C mutex, and `rx_socket` is guarded by
// `accept`.
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
            accept: Mutex::new(()),
            send: Mutex::new(()),
            inbound_shutdown: CancellationToken::new(),
        }
    }

    /// Close the receive socket
    ///
    /// Stops all inbound streams. This does not wait for sends in progress.
    pub fn close_rx_socket(&self) {
        self.inbound_shutdown.cancel();
        let _accept = self.lock_accept();
        // SAFETY: the accept lock excludes concurrent accepts, the only other
        // users of `rx_socket`.
        unsafe { cspcl_sys::types::close_rx_socket_raw(self.inner.get()) };
    }

    /// Accept one inbound connection.
    ///
    /// The returned connection is closed when it is dropped. Returns
    /// [`Error::Timeout`] without accepting once the receive socket is closed.
    pub(crate) fn poll_connection(&self) -> Result<AcceptedConn> {
        let _accept = self.lock_accept();
        if self.inbound_shutdown.is_cancelled() {
            // Do not let the C accept reopen a socket closed on purpose.
            return Err(Error::Timeout);
        }
        // SAFETY: no other accept or close runs (accept lock), and
        // `cspcl_accept_conn` touches nothing that sends modify.
        from_sys_result(unsafe { cspcl_sys::types::accept_conn_raw(self.inner.get(), 10) })
    }

    /// Send a serialized bundle, blocking until the peer acknowledges it.
    ///
    /// If `cancelled` is set by the time this send's turn comes, the bundle
    /// is not sent and [`Error::Cancelled`] is returned.
    pub(crate) fn send_raw(
        &self,
        bundle: &[u8],
        dest: CspAddress,
        cancelled: Option<&AtomicBool>,
    ) -> Result<()> {
        // `()` cannot be left inconsistent by a panic, so ignore poisoning.
        let _send = self.send.lock().unwrap_or_else(PoisonError::into_inner);
        if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Acquire)) {
            return Err(Error::Cancelled);
        }
        // SAFETY: the instance is alive (we hold `&self`), and
        // `cspcl_send_bundle` locks the C connection pool internally.
        unsafe { cspcl_sys::types::send_bundle_raw(self.inner.get(), bundle, dest.addr, dest.port) }
            .map_err(Error::from_raw)
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        // SAFETY: `initialized` is only written by init and cleanup, never
        // while `&self` is reachable.
        unsafe { (*self.inner.get()).initialized }
    }

    fn lock_accept(&self) -> MutexGuard<'_, ()> {
        self.accept.lock().expect("CSPCL accept lock poisoned")
    }

    /// The instance-wide inbound shutdown token. Cancel only through
    /// [`close_rx_socket`](Self::close_rx_socket) or drop; streams must use
    /// child tokens.
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
