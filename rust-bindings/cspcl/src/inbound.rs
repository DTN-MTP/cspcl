use futures::{Stream, channel::mpsc};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::task::{self, JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, trace, warn};

use crate::{Bundle, Cspcl, Error, Result};

/// An inbound connection that carried no bundle for this long is closed.
///
/// libcsp v1.6 cannot tell whether the peer is still there, so without this a
/// connection from a peer that went away would be held forever, eventually
/// exhausting libcsp's fixed connection table. A peer that later reuses its
/// pooled connection gets an immediate send failure and reconnects.
const INBOUND_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// At most this many inbound connections are served at once, like the C
/// receive table. When a new connection arrives beyond that, the least
/// recently active one is closed (its peer reconnects on its next send).
/// libcsp's connection table is sized for this many inbound connections.
const MAX_INBOUND_CONNS: usize = cspcl_sys::CSPCL_RX_CONN_TABLE_SIZE as usize;

/// The accept loop's view of a running listener.
struct ListenerHandle {
    shutdown: CancellationToken,
    /// When the listener last received a bundle (or was started). The
    /// listener holds the other reference while it runs.
    last_active: Arc<Mutex<Instant>>,
}

impl ListenerHandle {
    fn is_running(&self) -> bool {
        Arc::strong_count(&self.last_active) > 1
    }

    fn last_active(&self) -> Instant {
        *lock(&self.last_active)
    }
}

/// Running listeners, by the (address, port) their connection comes from.
#[derive(Default)]
struct ListenerTable {
    listeners: HashMap<(u8, u8), ListenerHandle>,
}

impl ListenerTable {
    /// Register the listener for a newly accepted connection, closing the
    /// connections it supersedes. Returns the activity timestamp the new
    /// listener must hold while it runs.
    fn admit(&mut self, key: (u8, u8), shutdown: CancellationToken) -> Arc<Mutex<Instant>> {
        self.listeners.retain(|_, listener| listener.is_running());

        // libcsp gives every outgoing connection a fresh ephemeral source
        // port, so (address, port) identifies one connection, and agents
        // sharing a CSP address never collide. The same pair only reappears
        // once the peer's port counter wrapped or it restarted: the old
        // connection is then dead.
        if let Some(stale) = self.listeners.remove(&key) {
            debug!(
                src_addr = key.0,
                src_port = key.1,
                "Connection reopened from the same address and port, closing the old one"
            );
            stale.shutdown.cancel();
        }

        if self.listeners.len() >= MAX_INBOUND_CONNS {
            let least_recent = self
                .listeners
                .iter()
                .min_by_key(|(_, listener)| listener.last_active())
                .map(|(key, _)| *key);
            if let Some(evicted) = least_recent.and_then(|key| self.listeners.remove(&key)) {
                debug!(
                    "{MAX_INBOUND_CONNS} inbound connections open, closing the least recently active one"
                );
                evicted.shutdown.cancel();
            }
        }

        let last_active = Arc::new(Mutex::new(Instant::now()));
        self.listeners.insert(
            key,
            ListenerHandle {
                shutdown,
                last_active: Arc::clone(&last_active),
            },
        );
        last_active
    }
}

/// Lock a mutex whose plain-data contents cannot be left inconsistent.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

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
        // A child token: dropping this stream stops only this stream, while
        // closing the receive socket or dropping the instance stops all.
        let inbound_shutdown = cspcl.inbound_shutdown_token().child_token();
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

    let mut listeners = ListenerTable::default();

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

        let last_active =
            listeners.admit((conn.src_addr, conn.src_port), listener_shutdown.clone());
        bundle_listeners
            .lock()
            .expect("Inbound bundle listener task set lock poisoned")
            .spawn_blocking({
                let conn = OwnedConn {
                    conn,
                    _cspcl: Arc::clone(&cspcl),
                };
                move || listen_incoming_from_conn(conn, tx, listener_shutdown, last_active)
            });
    }

    debug!("Inbound connection polling task stopped");
}

/// An accepted connection together with the instance it belongs to.
///
/// Fields drop in declaration order, so the connection is always closed
/// before the instance can be cleaned up, even if the listener task is
/// dropped without ever running.
struct OwnedConn {
    conn: cspcl_sys::types::AcceptedConn,
    _cspcl: Arc<Cspcl>,
}

fn listen_incoming_from_conn(
    owned: OwnedConn,
    tx: mpsc::UnboundedSender<Result<Bundle>>,
    inbound_shutdown: CancellationToken,
    last_active: Arc<Mutex<Instant>>,
) {
    let conn = &owned.conn;
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
                *lock(&last_active) = Instant::now();
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
                if lock(&last_active).elapsed() >= INBOUND_IDLE_TIMEOUT {
                    debug!(
                        src_addr = conn.src_addr,
                        src_port = conn.src_port,
                        "Inbound connection idle for {INBOUND_IDLE_TIMEOUT:?}, closing it"
                    );
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
    // Dropping `owned` closes the connection, then releases the instance.
}

fn recv_bundle_from_conn(conn: &cspcl_sys::types::AcceptedConn) -> Result<Bundle> {
    let received = cspcl_sys::types::recv_bundle_from_conn(conn).map_err(Error::from_raw)?;

    Ok(Bundle {
        data: received.data().to_vec(),
        src_addr: received.src_addr,
        src_port: received.src_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Admit a listener and simulate it running by keeping its timestamp.
    fn admit(table: &mut ListenerTable, key: (u8, u8)) -> (CancellationToken, Arc<Mutex<Instant>>) {
        let shutdown = CancellationToken::new();
        let running = table.admit(key, shutdown.clone());
        (shutdown, running)
    }

    #[test]
    fn same_address_different_ports_coexist() {
        let mut table = ListenerTable::default();
        let (first, _first_running) = admit(&mut table, (5, 20));
        let (second, _second_running) = admit(&mut table, (5, 21));

        assert!(!first.is_cancelled());
        assert!(!second.is_cancelled());
    }

    #[test]
    fn reopened_connection_replaces_the_old_one() {
        let mut table = ListenerTable::default();
        let (old, _old_running) = admit(&mut table, (5, 20));
        let (new, _new_running) = admit(&mut table, (5, 20));

        assert!(old.is_cancelled());
        assert!(!new.is_cancelled());
    }

    #[test]
    fn least_recently_active_is_evicted_when_full() {
        let mut table = ListenerTable::default();
        let admitted: Vec<_> = (0..MAX_INBOUND_CONNS as u8)
            .map(|port| admit(&mut table, (5, port)))
            .collect();
        // Every listener but the first has received a bundle since.
        let later = Instant::now() + Duration::from_secs(1);
        for (_, running) in &admitted[1..] {
            *lock(running) = later;
        }

        let (newest, _newest_running) = admit(&mut table, (6, 0));

        assert!(admitted[0].0.is_cancelled(), "least recently active kept");
        assert!(
            admitted[1..].iter().all(|(token, _)| !token.is_cancelled()),
            "an active listener was evicted"
        );
        assert!(!newest.is_cancelled());
    }

    #[test]
    fn finished_listeners_do_not_count() {
        let mut table = ListenerTable::default();
        for port in 0..MAX_INBOUND_CONNS as u8 {
            // The listener's timestamp is dropped at once: it has finished.
            let _ = admit(&mut table, (5, port));
        }

        let (newest, _newest_running) = admit(&mut table, (6, 0));

        assert!(!newest.is_cancelled());
        assert_eq!(table.listeners.len(), 1);
    }
}
