use std::cell::UnsafeCell;
use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use cspcl_sys::types::AcceptedConn;

use crate::{
    CspAddress,
    error::{Error, Result, from_sys_result},
    inbound::Receiver,
    sync::lock,
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
    /// The inbound side, see [`InboundStream`](crate::InboundStream).
    pub(crate) inbound: Receiver,
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
            inbound: Receiver::new(),
        }
    }

    /// Close the receive socket
    ///
    /// Ends all open inbound streams, as if they were all dropped: they yield
    /// `None` once the bundles already in them are read, and receiving stops
    /// (see [`inbound`](Self::inbound) for what happens to reads and
    /// connections then). A stream opened later receives again.
    ///
    /// The CSP port stays bound for the life of the process: libcsp 1.6 has
    /// no safe unbind, so the C side only forgets the socket. Peers can
    /// still connect and send; their sends are not refused but time out
    /// (after twice the ack timeout). Connections queued meanwhile are
    /// accepted by the next stream, which then receives bundles whose sender
    /// already reported a timeout and may send them again.
    ///
    /// This does not wait for sends in progress, but blocks while an accept
    /// poll is in progress (at most `CSPCL_RX_ACCEPT_POLL_MS`, 100 ms by
    /// default), which can be the case while a
    /// stream is open or just after the last one closed: from async code,
    /// call it with `tokio::task::spawn_blocking` where such a pause matters.
    pub fn close_rx_socket(&self) {
        // Stop receiving first: the accept loop then leaves the accept lock
        // after its current poll instead of retaking it, and cannot reopen
        // the socket. New streams wait until the socket is closed. The
        // receiver state is not held while waiting for the accept lock, so
        // listeners and streams are not blocked.
        let _closing = self.inbound.begin_close();
        let _accept = self.lock_accept();
        // SAFETY: the accept lock excludes concurrent accepts, the only other
        // users of `rx_socket`.
        unsafe { cspcl_sys::types::close_rx_socket_raw(self.inner.get()) };
    }

    /// Accept one inbound connection.
    ///
    /// The returned connection is closed when it is dropped. Returns
    /// [`Error::Timeout`] without accepting while no stream is open: the C
    /// accept reopens a closed receive socket, which must not happen after
    /// [`close_rx_socket`](Self::close_rx_socket).
    pub(crate) fn poll_connection(&self, timeout: Duration) -> Result<AcceptedConn> {
        let _accept = self.lock_accept();
        if !self.inbound.is_receiving() {
            return Err(Error::Timeout);
        }
        // SAFETY: no other accept or close runs (accept lock), and
        // `cspcl_accept_conn` touches nothing that sends modify.
        let timeout_ms = u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX)
            .max(1);
        from_sys_result(unsafe { cspcl_sys::types::accept_conn_raw(self.inner.get(), timeout_ms) })
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
        let _send = lock(&self.send);
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
        lock(&self.accept)
    }
}

impl Drop for Cspcl {
    fn drop(&mut self) {
        // No receiver thread is left: they all hold an `Arc<Cspcl>`.
        cspcl_sys::types::cleanup(self.inner.get_mut());
    }
}
