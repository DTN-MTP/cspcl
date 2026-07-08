use crate::{Bundle, Cspcl, Error, InboundStream, Result};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, mpsc},
    task::{self, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, warn};

pub type BundleSender = mpsc::UnboundedSender<Bundle>;
pub type AdvertisingConnectionChannel = mpsc::UnboundedSender<()>;

#[derive(Clone)]
pub struct InboundListener {
    cspcl: Arc<Cspcl>,
    bundle_sender: BundleSender,
    advertising_connection_sender: AdvertisingConnectionChannel,
    bundle_listeners: Arc<Mutex<JoinSet<()>>>,
    inbound_shutdown: CancellationToken,
}

pub struct ConnectionEvent {}

impl InboundListener {
    pub fn new(cspcl: Arc<Cspcl>) -> Self {
        let (bundle_sender, bundle_receiver) = mpsc::unbounded_channel::<Bundle>();
        let (advertising_connection_sender, advertising_connection_receiver) =
            mpsc::unbounded_channel::<()>();
        let bundle_listeners = Arc::new(Mutex::new(JoinSet::new()));
        let _ = InboundStream::new(bundle_receiver);
        Self {
            cspcl: cspcl.clone(),
            bundle_listeners,
            bundle_sender,
            advertising_connection_sender,
            inbound_shutdown: cspcl.inbound_shutdown_token(),
        }
    }

    async fn poll_inbound_connections(&self) {
        debug!("Inbound connection polling task started");
        let cspcl = self.cspcl.clone();
        let inbound_shutdown = self.inbound_shutdown.clone();
        let listener = self.clone();

        task::spawn(async move {
            loop {
                if inbound_shutdown.is_cancelled() {
                    break;
                }

                debug!("Polling incoming connection");
                let conn = match cspcl.poll_connection() {
                    Ok(conn) => conn,
                    Err(Error::Timeout) => {
                        if inbound_shutdown.is_cancelled() {
                            break;
                        }
                        debug!("Timed out while waiting for new connection");
                        tokio::select! {
                            () = inbound_shutdown.cancelled() => break,
                            () = tokio::time::sleep(Duration::from_millis(500)) => {}
                        }
                        continue;
                    }
                    Err(error) => {
                        error!(
                            "Got error while waiting for new connection: {}",
                            error.to_string()
                        );
                        continue;
                    }
                };

                debug!(
                    "New connection made with {}:{}",
                    conn.src_addr, conn.src_port
                );
                listener.clone().listen_incoming_from_conn(conn).await;
            }
        });
    }

    async fn listen_incoming_from_conn(&self, conn: cspcl_sys::types::AcceptedConn) {
        let tx = self.bundle_sender.clone();
        let inbound_shutdown = self.inbound_shutdown.clone();
        let mut bundle_listeners = self.bundle_listeners.lock().await;
        debug!(
            src_addr = conn.src_addr,
            src_port = conn.src_port,
            "Inbound connection listener task started"
        );

        bundle_listeners.spawn_blocking(move || {
            loop {
                if inbound_shutdown.is_cancelled() {
                    break;
                }

                match recv_bundle_from_conn(conn) {
                    Ok(bundle) => {
                        debug!(
                            "Received bundle from {}:{}",
                            bundle.src_addr, bundle.src_port
                        );
                        if tx.send(bundle).is_err() {
                            warn!("Could not pass bundle to stream");
                            break;
                        }
                    }
                    Err(Error::Timeout) => {
                        debug!("Timed out while waiting for new bundle");
                        continue;
                    }
                    Err(error) => {
                        error!(
                            "Got error while trying to receive bundle: {}",
                            error.to_string()
                        );
                        break;
                    }
                }
            }
        });
    }
}

fn recv_bundle_from_conn(conn: cspcl_sys::types::AcceptedConn) -> Result<Bundle> {
    let received = cspcl_sys::types::recv_bundle_from_conn(conn).map_err(Error::from_raw)?;

    Ok(Bundle {
        data: received.data().to_vec(),
        src_addr: received.src_addr,
        src_port: received.src_port,
    })
}
