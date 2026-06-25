use cspcl_bindings::{CspAddress, InboundStream};
use futures_util::TryStreamExt;
use hardy_async::CancellationToken;
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, warn};

use hardy_bpa::{
    Bytes,
    cla::{ClaAddress, Sink},
};
use hardy_bpv7::eid::NodeId;
use tokio::task::{self, JoinHandle};

pub struct Runtime {
    pub csp_to_endpoint: HashMap<CspAddress, NodeId>,
    polling_task: Option<JoinHandle<()>>,
    cancel_polling_task: CancellationToken,
}

impl Runtime {
    pub fn new(csp_to_endpoint: HashMap<CspAddress, NodeId>) -> Self {
        Self {
            csp_to_endpoint,
            polling_task: None,
            cancel_polling_task: CancellationToken::new(),
        }
    }

    pub fn stop(&self) {
        self.cancel_polling_task.cancel();
    }

    pub async fn start_inbound(&mut self, sink: Arc<dyn Sink>, mut inbound: InboundStream) {
        let csp_to_endpoint = self.csp_to_endpoint.clone();

        let csp_to_addr_iter = self.csp_to_endpoint.iter();
        for csp_node in csp_to_addr_iter {
            let raw_addr: Bytes = Into::into(*csp_node.0);
            match sink
                .add_peer(ClaAddress::Private(raw_addr), &[csp_node.1.clone()])
                .await
            {
                Ok(true) => debug!(
                    "Registered CSP peer {}:{} as {}",
                    csp_node.0.addr, csp_node.0.port, csp_node.1
                ),
                Ok(false) => debug!(
                    "CSP peer {}:{} was already registered",
                    csp_node.0.addr, csp_node.0.port
                ),
                Err(e) => warn!(
                    "Failed to register CSP peer {}:{} as {}: {e}",
                    csp_node.0.addr, csp_node.0.port, csp_node.1
                ),
            }
        }
        let cancel_token = self.cancel_polling_task.child_token();

        info!("Starting polling task of inbound bundle stream");

        let polling_task = task::spawn(async move {
            loop {
                debug!("Polling Hardy CSPCL inbound stream");
                let next_bundle = tokio::select! {
                    _ = cancel_token.cancelled() => break,
                    next_bundle = inbound.try_next() => next_bundle,
                };
                let bundle = match next_bundle {
                    Ok(bundle) => match bundle {
                        Some(bundle) => bundle,
                        None => {
                            debug!("Hardy CSPCL inbound stream closed");
                            break;
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
        });
        self.polling_task = Some(polling_task);
    }
}
