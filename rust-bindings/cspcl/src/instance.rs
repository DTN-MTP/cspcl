use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::bundle::{Receiver, Sender};
use crate::cspcl_sys;
use crate::error::{Error, Result};
use crate::interface::Interface;

pub(crate) struct RawCspcl {
    inner: UnsafeCell<cspcl_sys::cspcl_t>,
    recv_lock: Mutex<()>,
    lifecycle_lock: Mutex<()>,
    closed: AtomicBool,
}

impl RawCspcl {
    fn new(local_addr: u8, local_port: u8, interface: Interface) -> Result<Self> {
        let mut cspcl = cspcl_sys::cspcl_t {
            local_addr,
            csp_port: local_port,
            iface_type: interface.clone().into(),
            ..Default::default()
        };

        match interface {
            Interface::Zmq(interface_name) => cspcl.zmqhub_addr = interface_name.into(),
            Interface::Can(interface_name) => cspcl.can_iface = interface_name.into(),
            Interface::Loopback(_) => {}
        }

        unsafe {
            Error::from_code(cspcl_sys::cspcl_init(&mut cspcl))?;
        }

        Ok(Self {
            inner: UnsafeCell::new(cspcl),
            recv_lock: Mutex::new(()),
            lifecycle_lock: Mutex::new(()),
            closed: AtomicBool::new(false),
        })
    }

    fn as_mut_ptr(&self) -> *mut cspcl_sys::cspcl_t {
        self.inner.get()
    }

    fn local_addr(&self) -> u8 {
        unsafe { (*self.as_mut_ptr()).local_addr }
    }

    fn local_port(&self) -> u8 {
        unsafe { (*self.as_mut_ptr()).csp_port }
    }

    fn is_initialized(&self) -> bool {
        !self.closed.load(Ordering::Acquire) && unsafe { (*self.as_mut_ptr()).initialized }
    }

    fn shutdown(&self) {
        let _guard = self.lifecycle_lock.lock().expect("lifecycle lock poisoned");

        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }

        unsafe {
            cspcl_sys::cspcl_cleanup(self.as_mut_ptr());
        }
    }

    fn ensure_initialized(&self) -> Result<()> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(Error::from_code(cspcl_sys::cspcl_error_t_CSPCL_ERR_NOT_INITIALIZED).unwrap_err())
        }
    }
}

impl Drop for RawCspcl {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// Safety: lifecycle is owned by Arc<RawCspcl>; the C implementation guards
// the outbound connection pool internally, and recv operations are serialized
// through recv_lock.
unsafe impl Send for RawCspcl {}
unsafe impl Sync for RawCspcl {}

/// Immutable runtime configuration for a CSPCL node.
#[derive(Debug, Clone)]
pub struct CspclConfig {
    local_addr: u8,
    local_port: u8,
    interface: Interface,
}

impl CspclConfig {
    pub fn new(local_addr: u8) -> Self {
        Self {
            local_addr,
            local_port: cspcl_sys::CSPCL_PORT_BP as u8,
            interface: Interface::default(),
        }
    }

    pub fn with_port(mut self, local_port: u8) -> Self {
        self.local_port = local_port;
        self
    }

    pub fn with_interface(mut self, interface: Interface) -> Self {
        self.interface = interface;
        self
    }
}

/// Bootstrap handle for a CSPCL instance.
///
/// `Cspcl` owns the underlying native instance and can be cheaply cloned into
/// dedicated send/receive handles through `split`, `sender`, and `receiver`.
#[derive(Clone)]
pub struct Cspcl {
    raw: Arc<RawCspcl>,
}

impl Cspcl {
    /// Initialize a new CSPCL instance with explicit port and interface settings.
    pub fn new(local_addr: u8, local_port: u8, interface: Interface) -> Result<Self> {
        Self::from_config(
            CspclConfig::new(local_addr)
                .with_port(local_port)
                .with_interface(interface),
        )
    }

    /// Initialize a new CSPCL instance from a reusable config.
    pub fn from_config(config: CspclConfig) -> Result<Self> {
        Ok(Self {
            raw: Arc::new(RawCspcl::new(
                config.local_addr,
                config.local_port,
                config.interface,
            )?),
        })
    }

    /// Create a sender handle that can be cloned and shared across tasks/threads.
    pub fn sender(&self) -> Sender {
        Sender::new(Arc::clone(&self.raw))
    }

    /// Create a receiver handle for blocking bundle reception.
    pub fn receiver(&self) -> Receiver {
        Receiver::new(Arc::clone(&self.raw))
    }

    /// Produce independent sender and receiver handles from the same native instance.
    pub fn split(&self) -> (Sender, Receiver) {
        (self.sender(), self.receiver())
    }

    /// Convenience method for single-handle send usage.
    pub fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
        self.sender().send_bundle(bundle, dest_addr, dest_port)
    }

    /// Convenience method for single-handle receive usage.
    pub fn recv_bundle(&self, timeout_ms: u32) -> Result<crate::bundle::ReceivedBundle> {
        self.receiver().recv_bundle(timeout_ms)
    }

    /// Convenience method for receiving into a caller-provided buffer.
    pub fn recv_bundle_into(
        &self,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<crate::bundle::ReceivedBundleView> {
        self.receiver().recv_bundle_into(buffer, timeout_ms)
    }

    pub fn shutdown(&self) -> Result<()> {
        self.raw.shutdown();
        Ok(())
    }

    pub fn local_addr(&self) -> u8 {
        self.raw.local_addr()
    }

    pub fn local_port(&self) -> u8 {
        self.raw.local_port()
    }

    pub fn is_initialized(&self) -> bool {
        self.raw.is_initialized()
    }
}

pub(crate) fn recv_lock(raw: &Arc<RawCspcl>) -> &Mutex<()> {
    &raw.recv_lock
}

pub(crate) fn ensure_initialized(raw: &Arc<RawCspcl>) -> Result<()> {
    raw.ensure_initialized()
}

pub(crate) fn raw_ptr(raw: &Arc<RawCspcl>) -> *mut cspcl_sys::cspcl_t {
    raw.as_mut_ptr()
}

pub(crate) type SharedRawCspcl = Arc<RawCspcl>;
