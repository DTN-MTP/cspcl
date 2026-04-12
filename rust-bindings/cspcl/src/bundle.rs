use crate::cspcl_sys;
use crate::error::{Error, Result};
use crate::instance::{ConnectionStats, SharedRawCspcl, ensure_initialized, raw_ptr, recv_lock};

/// Metadata and payload for a bundle received from CSPCL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedBundle {
    pub data: Vec<u8>,
    pub src_addr: u8,
    pub src_port: u8,
}

/// Metadata for a bundle written into a caller-provided buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceivedBundleView {
    pub len: usize,
    pub src_addr: u8,
    pub src_port: u8,
}

/// Shared outbound handle backed by the native CSPCL connection pool.
#[derive(Clone)]
pub struct Sender {
    raw: SharedRawCspcl,
}

impl Sender {
    pub(crate) fn new(raw: SharedRawCspcl) -> Self {
        Self { raw }
    }

    /// Send a serialized bundle to a destination CSP node/port.
    pub fn send_bundle(&self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
        if bundle.is_empty() {
            return Err(
                Error::from_code(cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM).unwrap_err(),
            );
        }
        ensure_initialized(&self.raw)?;

        unsafe {
            Error::from_code(cspcl_sys::cspcl_send_bundle(
                raw_ptr(&self.raw),
                bundle.as_ptr(),
                bundle.len(),
                dest_addr,
                dest_port,
            ))?;
        }
        Ok(())
    }

    pub fn connection_stats(&self) -> ConnectionStats {
        self.raw.connection_stats()
    }
}

/// Blocking receive handle. Concurrent receives are serialized per instance.
#[derive(Clone)]
pub struct Receiver {
    raw: SharedRawCspcl,
}

impl Receiver {
    pub(crate) fn new(raw: SharedRawCspcl) -> Self {
        Self { raw }
    }

    /// Receive a bundle with the provided timeout in milliseconds.
    pub fn recv_bundle(&self, timeout_ms: u32) -> Result<ReceivedBundle> {
        let mut buffer = vec![0u8; cspcl_sys::CSPCL_MAX_BUNDLE_SIZE as usize];
        let received = self.recv_bundle_into(&mut buffer, timeout_ms)?;
        buffer.truncate(received.len);
        Ok(ReceivedBundle {
            data: buffer,
            src_addr: received.src_addr,
            src_port: received.src_port,
        })
    }

    /// Receive a bundle into a caller-provided buffer.
    pub fn recv_bundle_into(
        &self,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<ReceivedBundleView> {
        ensure_initialized(&self.raw)?;
        let _guard = recv_lock(&self.raw).lock().expect("receiver lock poisoned");

        let mut len = buffer.len();
        let mut src_addr: u8 = 0;
        let mut src_port: u8 = 0;

        unsafe {
            Error::from_code(cspcl_sys::cspcl_recv_bundle(
                raw_ptr(&self.raw),
                buffer.as_mut_ptr(),
                &mut len,
                &mut src_addr,
                &mut src_port,
                timeout_ms,
            ))?;
        }

        Ok(ReceivedBundleView {
            len,
            src_addr,
            src_port,
        })
    }
}
