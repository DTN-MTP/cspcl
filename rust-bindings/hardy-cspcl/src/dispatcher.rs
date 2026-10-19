use cspcl_bindings::CspAddress;
use futures_util::TryStreamExt;
use hardy_async::{CancellationToken, JoinHandle};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

use hardy_bpa::{
    Bytes,
    cla::{self, ClaAddress},
};
use hardy_bpv7::eid::NodeId;
use tokio::task::{self};

use crate::transport::{OpenError, Transport};

pub type DispatcherHandle = JoinHandle<()>;

/// First pause before reopening an inbound stream that ended, or could not
/// be opened, without the dispatcher being cancelled. It doubles on each
/// further failure, up to [`MAX_REOPEN_DELAY`], and starts over once a
/// stream stayed open for longer than the current delay.
///
/// Reopening only happens if cspcl's accept thread could not start or
/// failed. cspcl may have closed the inbound connections by then; peers
/// reconnect on their next send.
const REOPEN_DELAY: Duration = Duration::from_millis(500);

const MAX_REOPEN_DELAY: Duration = Duration::from_secs(30);

pub struct Dispatcher {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    cancel_polling_task: CancellationToken,
    transport: Transport,
    sink: Arc<dyn cla::Sink>,
}

impl Dispatcher {
    pub fn new(
        csp_to_endpoint: HashMap<CspAddress, NodeId>,
        cancel_polling_task: CancellationToken,
        transport: Transport,
        sink: Arc<dyn cla::Sink>,
    ) -> Self {
        Self {
            csp_to_endpoint,
            cancel_polling_task,
            transport,
            sink,
        }
    }

    pub fn start_dispatch_inbound_bundle(self) -> DispatcherHandle {
        let csp_to_endpoint = self.csp_to_endpoint.clone();
        let cancel_token = self.cancel_polling_task.child_token();

        info!("Starting polling task of inbound bundle stream");
        let sink = self.sink.clone();

        task::spawn(async move {
            let mut inbound = None;
            let mut opened_at = Instant::now();
            let mut backoff = Backoff::new();
            loop {
                let stream = match &mut inbound {
                    Some(stream) => stream,
                    None => {
                        // A stream opened after the cancel is dropped here.
                        let opened = tokio::select! {
                            biased;
                            _ = cancel_token.cancelled() => break,
                            opened = self.transport.inbound_stream() => opened,
                        };
                        match opened {
                            Ok(stream) => {
                                opened_at = Instant::now();
                                inbound.insert(stream)
                            }
                            Err(OpenError::Closed) => break,
                            Err(OpenError::Failed) => {
                                warn!(
                                    "Could not open the Hardy CSPCL inbound stream, retrying in {:?}",
                                    backoff.delay
                                );
                                if !backoff.wait(&cancel_token).await {
                                    break;
                                }
                                continue;
                            }
                        }
                    }
                };

                debug!("Polling Hardy CSPCL inbound stream");
                let next_bundle = tokio::select! {
                    // Unregistering cancels, then ends the stream: take the
                    // cancel, not the end, for what it is.
                    biased;
                    _ = cancel_token.cancelled() => break,
                    next_bundle = stream.try_next() => next_bundle,
                };
                let bundle = match next_bundle {
                    Ok(bundle) => match bundle {
                        Some(bundle) => bundle,
                        None => {
                            // Not cancelled: cspcl's accept thread could not
                            // start or failed. Without a stream, no bundle
                            // would be received again until the CLA is
                            // registered anew.
                            inbound = None;
                            if opened_at.elapsed() > backoff.delay {
                                // Not a quick failure: earlier ones are over.
                                backoff = Backoff::new();
                            }
                            warn!(
                                "Hardy CSPCL inbound stream ended, reopening it in {:?}",
                                backoff.delay
                            );
                            if !backoff.wait(&cancel_token).await {
                                break;
                            }
                            continue;
                        }
                    },
                    Err(e) => {
                        warn!("Error occured when receiving bundle: {}", e.to_string());
                        continue;
                    }
                };
                debug!(
                    len = bundle.data.len(),
                    "New bundle in inbound stream from {}:{}", bundle.src_addr, bundle.src_port
                );
                let bundle_data: Bytes = bundle.data.into();
                let csp_peer_addr = CspAddress {
                    addr: bundle.src_addr,
                    port: bundle.src_port,
                };
                let node_id = csp_to_endpoint.get(&csp_peer_addr);
                match sink
                    .dispatch(
                        bundle_data,
                        node_id,
                        Some(&ClaAddress::Private(csp_peer_addr.into())),
                    )
                    .await
                {
                    Ok(()) => info!(
                        peer_node = node_id.map(|node_id| node_id.to_string()),
                        "Dispatched inbound CSP bundle from {}:{} to BPA",
                        csp_peer_addr.addr,
                        csp_peer_addr.port
                    ),
                    Err(e) => warn!(
                        "Failed to dispatch inbound CSP bundle from {}:{} to BPA: {e}",
                        csp_peer_addr.addr, csp_peer_addr.port
                    ),
                }
            }
        })
    }
}

/// The pause before the next attempt to open an inbound stream.
struct Backoff {
    delay: Duration,
}

impl Backoff {
    fn new() -> Self {
        Self {
            delay: REOPEN_DELAY,
        }
    }

    /// Wait the current delay, then double it for the next failure.
    /// Returns `false` if cancelled first.
    async fn wait(&mut self, cancel_token: &CancellationToken) -> bool {
        let delay = self.delay;
        self.delay = (delay * 2).min(MAX_REOPEN_DELAY);
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => false,
            _ = tokio::time::sleep(delay) => true,
        }
    }
}
