use futures::{Stream, channel::mpsc};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::task;
use tracing::{debug, error, trace, warn};

use crate::{Bundle, Cspcl, Error, Result};

pub struct InboundStream {
    rx: mpsc::UnboundedReceiver<Result<Bundle>>,
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
    pub async fn new(cspcl: Arc<Cspcl>) -> Self {
        let (tx, rx) = mpsc::unbounded();
        debug!("Creating inbound stream");
        tokio::spawn(async move { poll_inbound_connections(cspcl, tx).await });

        Self { rx }
    }
}

async fn poll_inbound_connections(cspcl: Arc<Cspcl>, tx: mpsc::UnboundedSender<Result<Bundle>>) {
    debug!("Inbound connection polling task started");

    loop {
        debug!("Polling incoming connection");
        let conn = match cspcl.poll_connection() {
            Ok(conn) => conn,
            Err(Error::Timeout) => {
                warn!("Timed out while waiting for new connection");
                tokio::time::sleep(Duration::from_millis(500)).await;
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
        debug!(
            "New connection made with {}:{}",
            conn.src_addr, conn.src_port
        );
        task::spawn_blocking(move || listen_incoming_from_conn(conn, tx));
    }

    debug!("Inbound connection polling task stopped");
}

fn listen_incoming_from_conn(
    conn: cspcl_sys::types::AcceptedConn,
    tx: mpsc::UnboundedSender<Result<Bundle>>,
) {
    debug!(
        src_addr = conn.src_addr,
        src_port = conn.src_port,
        "Inbound connection listener task started"
    );

    loop {
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
                warn!("Timed out while waiting for new bundle");
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
