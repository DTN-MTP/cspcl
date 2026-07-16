use cspcl_bindings::{CspAddress, InboundStream};
use futures_util::TryStreamExt;
use hardy_async::{CancellationToken, JoinHandle};
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info, warn};

use hardy_bpa::{
    Bytes,
    cla::{self, ClaAddress},
};
use hardy_bpv7::eid::NodeId;
use tokio::task::{self};

use crate::bundle_debug::bundle_label;

pub type DispatcherHandle = JoinHandle<()>;

pub struct Dispatcher {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    cancel_polling_task: CancellationToken,
    inbound: InboundStream,
    sink: Arc<dyn cla::Sink>,
}

impl Dispatcher {
    pub fn new(
        csp_to_endpoint: HashMap<CspAddress, NodeId>,
        cancel_polling_task: CancellationToken,
        inbound: InboundStream,
        sink: Arc<dyn cla::Sink>,
    ) -> Self {
        Self {
            csp_to_endpoint,
            cancel_polling_task,
            inbound,
            sink,
        }
    }

    pub fn start_dispatch_inbound_bundle(mut self) -> DispatcherHandle {
        let csp_to_endpoint = self.csp_to_endpoint.clone();
        let cancel_token = self.cancel_polling_task.child_token();

        info!("Starting polling task of inbound bundle stream");
        let sink = self.sink.clone();

        task::spawn(async move {
            loop {
                debug!("Polling Hardy CSPCL inbound stream");
                let next_bundle = tokio::select! {
                    _ = cancel_token.cancelled() => break,
                    next_bundle = self.inbound.try_next() => next_bundle,
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
                let label = bundle_label(&bundle.data);
                info!(
                    len = bundle.data.len(),
                    bundle = %label,
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
