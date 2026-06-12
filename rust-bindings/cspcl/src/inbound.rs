use futures::{Stream, channel::mpsc};
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread,
};

use crate::{Bundle, Cspcl, Error, Result};

pub struct InboundStream {
    rx: mpsc::UnboundedReceiver<Result<Bundle>>,
}

impl Stream for InboundStream {
    type Item = Result<Bundle>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

impl InboundStream {
    pub fn new(cspcl: Arc<Cspcl>) -> Self {
        let (tx, rx) = mpsc::unbounded();
        thread::spawn(move || poll_inbound_connections(cspcl, tx));

        Self { rx }
    }
}

fn poll_inbound_connections(cspcl: Arc<Cspcl>, tx: mpsc::UnboundedSender<Result<Bundle>>) {
    loop {
        let conn = match cspcl.poll_connection() {
            Ok(conn) => conn,
            Err(Error::Timeout) => continue,
            Err(error) => {
                if tx.unbounded_send(Err(error)).is_err() {
                    break;
                }
                continue;
            }
        };

        let tx = tx.clone();
        thread::spawn(move || listen_incoming_from_conn(conn, tx));
    }
}

fn listen_incoming_from_conn(
    conn: cspcl_sys::types::AcceptedConn,
    tx: mpsc::UnboundedSender<Result<Bundle>>,
) {
    loop {
        match recv_bundle_from_conn(conn) {
            Ok(bundle) => {
                if tx.unbounded_send(Ok(bundle)).is_err() {
                    break;
                }
            }
            Err(Error::Timeout) => continue,
            Err(error) => {
                let _ = tx.unbounded_send(Err(error));
                break;
            }
        }
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
