use futures::{Stream, channel::mpsc};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};
use tokio::task::{self, JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace, warn};

use crate::{Bundle, Cspcl, Error, Result};

pub struct InboundStream {
    rx: mpsc::UnboundedReceiver<Result<Bundle>>,
    connection_listener: JoinHandle<()>,
    bundle_listeners: Arc<Mutex<JoinSet<()>>>,
    inbound_shutdown: CancellationToken,
}

impl Stream for InboundStream {
    type Item = Result<Bundle>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        trace!("Polling inbound stream");
        let poll = Pin::new(&mut self.rx).poll_next(cx);

        match &poll {
            Poll::Ready(Some(Ok(bundle))) => trace!(
                src_addr = bundle.src_addr,
                src_port = bundle.src_port,
                bundle_len = bundle.data.len(),
                "Inbound stream yielded bundle"
            ),
            Poll::Ready(Some(Err(error))) => {
                trace!(%error, "Inbound stream yielded error");
            }
            Poll::Ready(None) => trace!("Inbound stream closed"),
            Poll::Pending => trace!("Inbound stream pending"),
        }

        poll
    }
}

impl InboundStream {
    pub fn new(cspcl: Arc<Cspcl>) -> Self {
        let (tx, rx) = mpsc::unbounded();
        debug!("Creating inbound stream");

        let bundle_listeners = Arc::new(Mutex::new(JoinSet::new()));
        let inbound_shutdown = cspcl.inbound_shutdown_token();
        let connection_listener = task::spawn({
            let bundle_listeners = Arc::clone(&bundle_listeners);
            let inbound_shutdown = inbound_shutdown.clone();
            async move { poll_inbound_connections(cspcl, tx, bundle_listeners, inbound_shutdown).await }
        });

        Self {
            rx,
            connection_listener,
            bundle_listeners,
            inbound_shutdown,
        }
    }
}

impl Drop for InboundStream {
    fn drop(&mut self) {
        debug!("Cancelling inbound stream tasks");
        self.inbound_shutdown.cancel();

        debug!("Aborting inbound connection polling task");
        self.connection_listener.abort();

        debug!("Aborting inbound bundle listener tasks");
        self.bundle_listeners
            .lock()
            .expect("Inbound bundle listener task set lock poisoned")
            .abort_all();
    }
}

async fn poll_inbound_connections(
    cspcl: Arc<Cspcl>,
    tx: mpsc::UnboundedSender<Result<Bundle>>,
    bundle_listeners: Arc<Mutex<JoinSet<()>>>,
    inbound_shutdown: CancellationToken,
) {
    debug!("Inbound connection polling task started");

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
                if tx.unbounded_send(Err(error)).is_err() {
                    debug!("Inbound stream receiver dropped while sending connection error");
                    break;
                }
                continue;
            }
        };

        let tx = tx.clone();
        let listener_shutdown = inbound_shutdown.child_token();
        debug!(
            "New connection made with {}:{}",
            conn.src_addr, conn.src_port
        );
        bundle_listeners
            .lock()
            .expect("Inbound bundle listener task set lock poisoned")
            .spawn_blocking(move || listen_incoming_from_conn(conn, tx, listener_shutdown));
    }

    debug!("Inbound connection polling task stopped");
}

fn listen_incoming_from_conn(
    conn: cspcl_sys::types::AcceptedConn,
    tx: mpsc::UnboundedSender<Result<Bundle>>,
    inbound_shutdown: CancellationToken,
) {
    debug!(
        src_addr = conn.src_addr,
        src_port = conn.src_port,
        "Inbound connection listener task started"
    );

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
                if tx.unbounded_send(Ok(bundle)).is_err() {
                    warn!("Could not pass bundle to stream");
                    break;
                }
                trace!("Bundle sent to inbound stream");
            }
            Err(Error::Timeout) => {
                if inbound_shutdown.is_cancelled() {
                    break;
                }
                debug!("Timed out while waiting for new bundle");
                continue;
            }
            Err(error) => {
                error!(
                    "Got error while trying to receive bundle: {}",
                    error.to_string()
                );
                if tx.unbounded_send(Err(error)).is_ok() {
                    trace!("Error sent to inbound stream");
                }
                break;
            }
        }
    }

    debug!(
        src_addr = conn.src_addr,
        src_port = conn.src_port,
        "Inbound connection listener task stopped"
    );
}

fn recv_bundle_from_conn(conn: cspcl_sys::types::AcceptedConn) -> Result<Bundle> {
    let received = cspcl_sys::types::recv_bundle_from_conn(conn).map_err(Error::from_raw)?;

    Ok(Bundle {
        data: received.data().to_vec(),
        src_addr: received.src_addr,
        src_port: received.src_port,
    })
}
