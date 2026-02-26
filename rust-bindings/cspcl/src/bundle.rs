use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;

use futures::channel::mpsc;
use futures::{SinkExt, Stream};

use crate::cspcl_sys;
use crate::error::{Error, Result};
use crate::instance::Cspcl;

/// A bundle received from the network: (data, src_addr, src_port)
pub type Bundle = (Vec<u8>, u8, u8);

/// An async `Stream` that yields received bundles.
///
/// Backed by a background thread that calls `recv_bundle` in a loop.
/// The thread exits automatically when this stream is dropped.
pub struct BundleStream {
    rx: mpsc::UnboundedReceiver<Result<Bundle>>,
}

impl Stream for BundleStream {
    type Item = Result<Bundle>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

impl Cspcl {
    /// Send a bundle to a remote CSP address
    ///
    /// This will automatically fragment the bundle if it exceeds CSP MTU
    /// and handle reassembly on the receiving end.
    ///
    /// # Arguments
    /// * `bundle` - The serialized bundle data to send
    /// * `dest_addr` - Destination CSP address
    /// * `dest_port` - Destination CSP port
    ///
    /// # Returns
    /// Ok(()) on success, or Err(Error) if the operation failed
    pub fn send_bundle(&mut self, bundle: &[u8], dest_addr: u8, dest_port: u8) -> Result<()> {
        if bundle.is_empty() {
            return Err(
                Error::from_code(cspcl_sys::cspcl_error_t_CSPCL_ERR_INVALID_PARAM).unwrap_err(),
            );
        }

        unsafe {
            Error::from_code(cspcl_sys::cspcl_send_bundle(
                self.inner_mut(),
                bundle.as_ptr(),
                bundle.len(),
                dest_addr,
                dest_port,
            ))?;
        }
        Ok(())
    }

    /// Receive a bundle with optional timeout
    ///
    /// This function blocks until a complete bundle is received or timeout occurs.
    /// Incomplete bundles are automatically reassembled.
    ///
    /// # Arguments
    /// * `timeout_ms` - Timeout in milliseconds (0 = no timeout)
    ///
    /// # Returns
    /// Ok((bundle_data, source_address, source_port)) on success
    /// Err(Error) if the operation timed out or failed
    pub fn recv_bundle(&mut self, timeout_ms: u32) -> Result<(Vec<u8>, u8, u8)> {
        // TODO: Consider making buffer size configurable via constructor or method parameter
        let mut buffer = vec![0u8; cspcl_sys::CSPCL_MAX_BUNDLE_SIZE as usize];
        let mut len = buffer.len();
        let mut src_addr: u8 = 0;
        let mut src_port: u8 = 0;
        unsafe {
            Error::from_code(cspcl_sys::cspcl_recv_bundle(
                self.inner_mut(),
                buffer.as_mut_ptr(),
                &mut len,
                &mut src_addr,
                &mut src_port,
                timeout_ms,
            ))?;
        }

        Ok((buffer, src_addr, src_port))
    }

    pub async fn bundle_stream(self) -> BundleStream {
        let (mut tx, rx) = mpsc::unbounded::<Result<Bundle>>();

        thread::spawn(async move || {
            let mut cspcl = self;
            loop {
                match cspcl.recv_bundle(100) {
                    Err(e) if e.code() == cspcl_sys::cspcl_error_t_CSPCL_ERR_TIMEOUT => {
                        if tx.is_closed() {
                            break;
                        }
                    }
                    result => {
                        if tx.send(result).await.is_err() {
                            tx.disconnect();
                            break;
                        };
                    }
                }
            }
        });

        BundleStream { rx }
    }
}
