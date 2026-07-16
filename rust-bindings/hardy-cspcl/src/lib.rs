mod bundle_debug;
mod cla;
mod config;
mod dispatcher;
mod liveness;
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
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

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

struct PeerRuntime {
    liveness: Arc<crate::liveness::PeerLiveness>,
    heartbeat: Duration,
    node_id: NodeId,
}

pub struct Cla {
    csp_to_endpoint: HashMap<CspAddress, NodeId>,
    peers: HashMap<CspAddress, PeerRuntime>,
    transport: transport::Transport,
    cancel_dispatcher: CancellationToken,
    sink: Once<Arc<dyn Sink>>,
    failure_threshold: u32,
    ping_timeout: Duration,
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

        let peers_config = config.peers.clone();
        let mut csp_to_endpoint = HashMap::<CspAddress, NodeId>::new();
        for peer in &peers_config {
            let csp_address = CspAddress {
                addr: peer.addr,
                port: peer.port,
            };
            csp_to_endpoint.insert(csp_address, peer.node_id.clone());
        }
        let transport = Transport::new(cspcl.clone());

        let mut peers = HashMap::<CspAddress, PeerRuntime>::new();
        for peer in &config.peers {
            let csp_address = CspAddress {
                addr: peer.addr,
                port: peer.port,
            };
            let heartbeat = Duration::from_secs(
                peer.heartbeat_interval
                    .unwrap_or(config.default_heartbeat_interval_s) as u64,
            );
            peers.insert(
                csp_address,
                PeerRuntime {
                    liveness: Arc::new(crate::liveness::PeerLiveness::new()),
                    heartbeat,
                    node_id: peer.node_id.clone(),
                },
            );
        }

        Ok(Self {
            csp_to_endpoint,
            peers,
            transport,
            cancel_dispatcher: CancellationToken::new(),
            sink: Once::new(),
            failure_threshold: config.failure_threshold,
            ping_timeout: Duration::from_millis(config.ping_timeout_ms as u64),
        })
    }

    fn sink(&self) -> Result<Arc<dyn Sink>, Error> {
        self.sink.get().cloned().ok_or(Error::Sink)
    }

    fn spawn_recovery(&self, csp_addr: CspAddress, peer: &PeerRuntime) {
        let Some(sink) = self.sink.get().cloned() else {
            warn!(
                "Cannot start recovery for {}:{}: sink not registered",
                csp_addr.addr, csp_addr.port
            );
            return;
        };

        let transport = self.transport.clone();
        let liveness = peer.liveness.clone();
        let heartbeat = peer.heartbeat;
        let node_ids = vec![peer.node_id.clone()];
        let cla_addr = ClaAddress::Private(Into::<Bytes>::into(csp_addr));
        let ping_timeout = self.ping_timeout;
        let dest_addr = csp_addr.addr;
        let cancel = self.cancel_dispatcher.child_token();

        info!(
            "Peer {}:{} down after {} failures; starting recovery probe",
            csp_addr.addr, csp_addr.port, self.failure_threshold
        );

        tokio::spawn(async move {
            crate::liveness::run_recovery(
                sink,
                cla_addr,
                node_ids,
                liveness,
                heartbeat,
                cancel,
                move || {
                    let transport = transport.clone();
                    async move {
                        tokio::task::spawn_blocking(move || {
                            transport.ping(dest_addr, ping_timeout).is_ok()
                        })
                        .await
                        .unwrap_or(false)
                    }
                },
            )
            .await;
        });
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
