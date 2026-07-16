use bytes::Bytes;
use cspcl_bindings::CspAddress;
use hardy_async::async_trait;
use hardy_bpa::cla::{self, ClaAddress, ClaAddressType, ForwardBundleResult};
use hardy_bpv7::eid::NodeId;
use tracing::warn;

use crate::Cla;

#[async_trait]
impl cla::Cla for Cla {
    fn address_type(&self) -> Option<ClaAddressType> {
        Some(ClaAddressType::Private)
    }

    async fn on_register(&self, sink: Box<dyn cla::Sink>, _node_ids: &[NodeId]) {
        self.sink.call_once(|| sink.into());
        self.register_peers()
            .await
            .expect("Sink must be registered at this point");
    }

    async fn on_unregister(&self) {
        self.cleanup().await;
    }

    async fn forward(
        &self,
        _queue: Option<u32>,
        cla_addr: &ClaAddress,
        bundle: Bytes,
    ) -> cla::Result<ForwardBundleResult> {
        let ClaAddress::Private(raw_addr) = cla_addr else {
            return Ok(ForwardBundleResult::NoNeighbour);
        };

        let csp_addr = CspAddress::try_from(raw_addr.clone())
            .map_err(|e| cla::Error::Internal(Box::new(e)))?;

        let result = self.transport.send_bundle(bundle, csp_addr).await;

        if let Some(peer) = self.peers.get(&csp_addr) {
            match &result {
                Ok(_) => peer.liveness.on_send_success(),
                Err(_) => {
                    if peer.liveness.on_send_failure(self.failure_threshold) {
                        self.spawn_recovery(csp_addr, peer);
                    }
                }
            }
        }

        match result {
            Ok(_) => Ok(ForwardBundleResult::Sent),
            Err(e) => {
                warn!(
                    "Failed to send CSP bundle to {}:{}: {e}",
                    csp_addr.addr, csp_addr.port
                );
                Err(cla::Error::Internal(Box::new(e)))
            }
        }
    }
}
