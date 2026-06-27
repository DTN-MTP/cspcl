mod cla;
mod config;
mod dispatcher;
mod transport;

pub use config::{Config, Interface, PeerConfig};

use cspcl_bindings::CspAddress;
use hardy_async::CancellationToken;
use hardy_async::sync::spin::{Once, RwLock};
use hardy_bpa::cla::Sink;
use hardy_bpv7::eid::NodeId;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

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
}

pub struct Cla {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    transport: transport::Transport,
    cancel_dispatcher: CancellationToken,
    sink: Once<Arc<dyn Sink>>,
    dispatcher: Once<Dispatcher>,
}

impl Cla {
    pub fn new(config: &Config) -> Result<Self, Error> {
        let interface: cspcl_bindings::Interface = match config.interface {
            Interface::Loopback => cspcl_bindings::Interface::Loopback,
            Interface::Can => cspcl_bindings::Interface::Can(config.interface_name.clone()),
        };

        let cspcl = Arc::new(RwLock::new(
            cspcl_bindings::Cspcl::new(
                CspAddress {
                    addr: config.local_addr,
                    port: config.port,
                },
                interface,
            )
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

        Ok(Self {
            csp_to_endpoint,
            transport,
            cancel_dispatcher: CancellationToken::new(),
            sink: Once::new(),
            dispatcher: Once::new(),
        })
    }

    pub fn build_dispatcher(&self) -> Result<&Dispatcher, Error> {
        self.dispatcher.get().map_or_else(
            || -> Result<&Dispatcher, Error> {
                let inbound = self.transport.inbound_stream();
                let sink = self
                    .sink
                    .get()
                    .ok_or(Error::Dispatcher("Sink is not initialiazed".to_string()))?;

                let dispatcher = self.dispatcher.call_once(|| {
                    Dispatcher::new(
                        self.csp_to_endpoint.clone(),
                        self.cancel_dispatcher.clone(),
                        inbound,
                        sink.clone(),
                    )
                });
                Ok(dispatcher)
            },
            Ok,
        )
    }

    pub async fn cleanup(&self) {
        debug!("Unregistering cspcl...");
        self.transport.cleanup();
        self.cancel_dispatcher.cancel();
    }
}
