use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;

use futures::channel::mpsc;
use futures::{SinkExt, Stream};

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
            return Err(Error::InvalidParam);
        }

        let mut inner = self.inner_mut();
        cspcl_sys::types::send_bundle(&mut inner, bundle, dest_addr, dest_port)
            .map_err(Error::from_raw)?;
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
        let mut inner = self.inner_mut();
        let received = cspcl_sys::types::recv_bundle(&mut inner, &mut buffer, timeout_ms)
            .map_err(Error::from_raw)?;
        let len = received.len;
        let src_addr = received.src_addr;
        let src_port = received.src_port;
        drop(received);
        buffer.truncate(len);

        Ok((buffer, src_addr, src_port))
    }

    pub async fn bundle_stream(self) -> BundleStream {
        let (mut tx, rx) = mpsc::unbounded::<Result<Bundle>>();

        thread::spawn(async move || {
            let mut cspcl = self;
            loop {
                match cspcl.recv_bundle(100) {
                    Err(Error::Timeout) => {
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
