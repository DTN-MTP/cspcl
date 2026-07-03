mod cla;
mod config;
mod dispatcher;
mod transport;

use bytes::Bytes;
pub use config::{Config, Interface, PeerConfig};

use cspcl_bindings::CspAddress;
use hardy_async::CancellationToken;
use hardy_async::sync::spin::Once;
use hardy_bpa::cla::{ClaAddress, Sink};
use hardy_bpv7::eid::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use crate::dispatcher::Dispatcher;
use crate::transport::Transport;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("transport initialization failed: {0}")]
    Init(#[from] transport::Error),
    #[error("registration failed: {0}")]
    Registration(#[from] hardy_bpa::cla::Error),
    #[error("could not create cspcl: {0}")]
    CscplInit(#[from] cspcl_bindings::Error),
    #[error("Could not create dispatcher: {0}")]
    Dispatcher(String),
    #[error("Sink is not registered")]
    Sink,
}

pub struct Cla {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    transport: transport::Transport,
    cancel_dispatcher: CancellationToken,
    sink: Once<Arc<dyn Sink>>,
}

impl Cla {
    pub fn new(config: &Config) -> Result<Self, Error> {
        let interface: cspcl_bindings::Interface = match config.interface {
            Interface::Loopback => cspcl_bindings::Interface::Loopback,
            Interface::Can => cspcl_bindings::Interface::Can(config.interface_name.clone()),
        };

        let cspcl = Arc::new(
            cspcl_bindings::Cspcl::new(
                CspAddress {
                    addr: config.local_addr,
                    port: config.port,
                },
                interface,
            )
            .map_err(|e: cspcl_bindings::Error| Error::CscplInit(e))?,
        );

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

        Ok(Self {
            csp_to_endpoint,
            transport,
            cancel_dispatcher: CancellationToken::new(),
            sink: Once::new(),
        })
    }

    fn sink(&self) -> Result<Arc<dyn Sink>, Error> {
        self.sink.get().cloned().ok_or(Error::Sink)
    }

    pub fn start_dispatcher(&self) -> Result<JoinHandle<()>, Error> {
        let inbound = self.transport.inbound_stream();
        let sink = self.sink()?;
        Ok(Dispatcher::new(
            self.csp_to_endpoint.clone(),
            self.cancel_dispatcher.clone(),
            inbound,
            sink.clone(),
        )
        .start_dispatch_inbound_bundle())
    }

    async fn register_peers(&self) -> Result<(), Error> {
        let sink = self.sink()?;

        for csp_node in self.csp_to_endpoint.clone().iter() {
            match sink
                .add_peer(
                    ClaAddress::Private(Into::<Bytes>::into(*csp_node.0)),
                    std::slice::from_ref(csp_node.1),
                )
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
        Ok(())
    }

    pub async fn cleanup(&self) {
        debug!("Unregistering cspcl...");
        self.transport.cleanup();
        self.cancel_dispatcher.cancel();
    }
}
