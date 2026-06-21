mod config;
mod runtime;
mod transport;

pub use config::{Config, Interface, PeerConfig};

use hardy_async::async_trait;
use hardy_async::sync::spin::{Once, RwLock};
use hardy_bpa::Bytes;
use hardy_bpa::bpa::BpaRegistration;
use hardy_bpa::cla::{self, ClaAddress, ClaAddressType, CspAddress, ForwardBundleResult};
use hardy_bpv7::eid::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::runtime::Runtime;
use crate::transport::Transport;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("transport initialization failed: {0}")]
    Init(#[from] transport::Error),
    #[error("registration failed: {0}")]
    Registration(#[from] cla::Error),
    #[error("could not create cspcl: {0}")]
    CscplInit(#[from] cspcl_bindings::Error),
}

pub struct Cla {
    transport: transport::Transport,
    runtime: RwLock<runtime::Runtime>,
    sink: Once<Arc<dyn cla::Sink>>,
}

impl Cla {
    pub fn new(config: &Config) -> Result<Self, Error> {
        let interface: cspcl_bindings::Interface = match config.interface {
            Interface::Loopback => cspcl_bindings::Interface::Loopback,
            Interface::Can => cspcl_bindings::Interface::Can(config.interface_name.clone()),
        };

        let cspcl = Arc::new(RwLock::new(
            cspcl_bindings::Cspcl::new(config.local_addr, config.port, interface)
                .map_err(|e: cspcl_bindings::Error| Error::CscplInit(e))?,
        ));

        let peers = config.peers.clone();
        let mut csp_to_endpoint = HashMap::<CspAddress, NodeId>::new();
        for peer in peers {
            let csp_address = CspAddress {
                addr: peer.addr,
                port: peer.port,
            };
            csp_to_endpoint.insert(csp_address, peer.node_id.clone());
        }

        let transport = Transport::new(cspcl.clone());
        let runtime = RwLock::new(Runtime::new(csp_to_endpoint));

        Ok(Self {
            transport,
            runtime,
            sink: Once::new(),
        })
    }

    pub async fn register(
        self: &Arc<Self>,
        bpa: &dyn BpaRegistration,
        name: String,
        policy: Option<Arc<dyn hardy_bpa::policy::EgressPolicy>>,
    ) -> Result<(), Error> {
        bpa.register_cla(name, self.clone(), policy).await?;
        Ok(())
    }

    pub async fn unregister(&self) {
        debug!("Unregistering cspcl...");
        self.transport.cleanup();
        self.runtime.write().stop();
    }
}

#[async_trait]
impl cla::Cla for Cla {
    fn address_type(&self) -> Option<ClaAddressType> {
        Some(ClaAddressType::Csp)
    }

    async fn on_register(&self, sink: Box<dyn cla::Sink>, _node_ids: &[NodeId]) {
        let sink: Arc<dyn cla::Sink> = sink.into();
        let sink = self.sink.call_once(|| sink);
        let inbound_stream = self.transport.inbound_stream().await;
        self.runtime
            .write()
            .start_inbound(sink.clone(), inbound_stream)
            .await;
    }

    async fn on_unregister(&self) {
        self.unregister().await;
    }

    async fn forward(
        &self,
        _queue: Option<u32>,
        cla_addr: &ClaAddress,
        bundle: Bytes,
    ) -> cla::Result<ForwardBundleResult> {
        let ClaAddress::Csp(csp_addr) = cla_addr else {
            return Ok(ForwardBundleResult::NoNeighbour);
        };
        match self
            .transport
            .send_bundle(bundle, csp_addr.addr, csp_addr.port)
            .await
        {
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
