//! Inbound bundles.
//!
//! Each [`Cspcl`] has one [`Receiver`], which receives while at least one
//! [`InboundStream`] is open: an accept thread, plus one listener thread per
//! inbound connection. Each bundle is handed to the most recently opened
//! stream.
//!
//! One thread per connection is needed because libcsp v1.6 raises every read
//! timeout on an RDP connection to the RDP connection timeout (10 s by
//! default): a single thread reading connections in turn, as the C
//! `cspcl_recv_bundle` does, would stall on each idle connection. The
//! connection table policy (replace a reopened connection, cap the number of
//! open connections) therefore lives here rather than in C. The C receive
//! table (`cspcl_rx_table_store`) serves only `cspcl_recv_bundle`, which the
//! Rust bindings do not use, so it stays empty for them; the two policies
//! are independent and differ (the slack while evictions finish, and the
//! reset and idle closes, exist only here).

use futures::{Stream, channel::mpsc};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::{Duration, Instant},
};
use tracing::{debug, error, trace, warn};

use crate::{
    Bundle, Cspcl, Error, Result,
    sync::{lock, wait_timeout, wait_timeout_while, wait_while},
};

/// At most this many inbound connections stay open, like the C receive
/// table. A new connection beyond that is accepted at once and the least
/// recently active other one is closed, which takes until its current read
/// returns.
const MAX_INBOUND_CONNS: usize = cspcl_sys::CSPCL_RX_CONN_TABLE_SIZE as usize;

/// Connections being closed may exceed [`MAX_INBOUND_CONNS`] by this many;
/// beyond that, new connections wait in libcsp's accept backlog. libcsp's
/// connection table has 4 spare slots beyond the outbound pool and the
/// receive table; this leaves 2 for the backlog.
const MAX_EXTRA_INBOUND_CONNS: usize = 2;

/// An inbound connection that carried no bundle for this long is closed. It
/// frees the slot of a peer that vanished without resetting the connection
/// (power loss, link cut), which libcsp cannot detect. A healthy peer idle
/// for longer reconnects on its next send.
const INBOUND_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// How long a receive that finds no bundle lasts. libcsp raises the read
/// timeout on an RDP connection (all inbound connections use RDP) to the
/// connection timeout announced by the peer when it connected. This assumes
/// the peer announces the same one as this node: libcsp's 10 s default,
/// which cspcl does not change.
fn expected_read_timeout() -> Duration {
    let requested = u64::from(cspcl_sys::CSPCL_SFP_TIMEOUT_MS);
    let rdp = u64::from(cspcl_sys::types::rdp_conn_timeout_ms());
    Duration::from_millis(requested.max(rdp))
}

/// Whether a receive that timed out after `elapsed` was instead woken by
/// libcsp closing the connection on a reset from the peer. That wake-up is
/// the only way libcsp ends a read early, and it comes only once: libcsp
/// keeps the connection open until it is closed here, and every later read
/// lasts the full timeout. A real timeout never ends early, so a tenth of
/// slack only absorbs clock imprecision. (`elapsed` covers the whole read,
/// which restarts the timeout for each fragment of a bundle: a reset in the
/// middle of a multi-fragment bundle is missed once the read has run past
/// about 90% of one timeout; the idle timeout covers it.)
fn woken_by_reset(elapsed: Duration, expected: Duration) -> bool {
    elapsed < expected.mul_f64(0.9)
}

/// Stack size of the receive threads. They hold no large buffer on the
/// stack, so the 2 MiB default would only reserve memory (up to 11 threads
/// per instance).
const RECEIVE_THREAD_STACK: usize = 256 * 1024;

/// How long one accept waits for a connection: the C `CSPCL_RX_ACCEPT_POLL_MS`.
/// It also bounds how long [`Cspcl::close_rx_socket`] may wait for an
/// accept in progress; its rustdoc is the one place that states this, other
/// docs point to it.
const ACCEPT_POLL: Duration = Duration::from_millis(cspcl_sys::CSPCL_RX_ACCEPT_POLL_MS as u64);

/// Pause before accepting again after an accept error, cut short when
/// receiving stops.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_secs(1);

/// After receiving stops (the last stream closed), listeners wait this long
/// without reading before they close their connection, so a stream closed
/// and reopened within it keeps all connections.
const STOP_GRACE: Duration = Duration::from_secs(1);

/// The inbound side of a [`Cspcl`] instance.
pub(crate) struct Receiver {
    state: Mutex<ReceiverState>,
    /// Signalled when receiving resumes and when the last close finishes.
    changed: Condvar,
    /// Origin of the listeners' activity timestamps.
    epoch: Instant,
    /// Shared with the listeners, which outlive the receiver when they hold
    /// the last reference to the instance.
    listeners: Arc<Listeners>,
}

pub(crate) struct ReceiverState {
    next_id: u64,
    /// Open streams, oldest first. The newest one gets the bundles.
    subscribers: Vec<(u64, mpsc::UnboundedSender<Result<Bundle>>)>,
    /// Bundles acknowledged to their sender while no stream was open: those
    /// left unread in a closed stream, and those whose read was in progress
    /// when receiving stopped (at most one per listener). Handed to the next
    /// stream; lost only if the instance is dropped first.
    pending: VecDeque<Bundle>,
    /// Whether an accept thread runs or is about to: set by `subscribe`
    /// before it spawns the thread (outside the lock), so a stream opened
    /// meanwhile starts no second one. Cleared under the lock when the
    /// thread decides to stop, when it panics, or when the spawn failed;
    /// streams opened while it was set count on that thread.
    accept_running: bool,
    /// When receiving last stopped; `None` while at least one stream is
    /// open. Listeners close their connection once it is
    /// [`STOP_GRACE`] old.
    stopped_at: Option<Instant>,
    /// Closes of the receive socket in progress; streams opened meanwhile
    /// wait for them, so they receive on the reopened socket.
    closing: usize,
}

impl ReceiverState {
    fn new() -> Self {
        Self {
            next_id: 0,
            subscribers: Vec::new(),
            pending: VecDeque::new(),
            accept_running: false,
            stopped_at: Some(Instant::now()),
            closing: 0,
        }
    }

    fn is_receiving(&self) -> bool {
        self.stopped_at.is_none()
    }

    /// The grace period starts when receiving goes from on to off, not again
    /// while it stays off. Callers that turn it on wake the listeners.
    fn set_receiving(&mut self, on: bool) {
        if on {
            self.stopped_at = None;
        } else {
            self.stopped_at.get_or_insert_with(Instant::now);
        }
    }

    /// Hand a bundle (or receive error) to the newest open stream.
    fn deliver(&mut self, item: Result<Bundle>) {
        // A stream is removed under this lock before its receiving end is
        // closed, so the newest one accepts the item.
        let item = match self.subscribers.last() {
            Some((_, tx)) => match tx.unbounded_send(item) {
                Ok(()) => return,
                Err(e) => e.into_inner(),
            },
            None => item,
        };
        match item {
            Ok(bundle) => {
                debug!(
                    "No inbound stream open, keeping bundle from {}:{} for the next one",
                    bundle.src_addr, bundle.src_port
                );
                self.pending.push_back(bundle);
            }
            Err(error) => debug!(%error, "No inbound stream open, dropping receive error"),
        }
    }
}

impl Receiver {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(ReceiverState::new()),
            changed: Condvar::new(),
            epoch: Instant::now(),
            listeners: Arc::default(),
        }
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    pub(crate) fn is_receiving(&self) -> bool {
        lock(&self.state).is_receiving()
    }

    /// Wait while receiving is stopped, for at most the rest of the grace
    /// period. Returns whether receiving is on, i.e. whether a listener
    /// should keep its connection.
    fn wait_while_stopped(&self) -> bool {
        let mut state = lock(&self.state);
        while let Some(stopped_at) = state.stopped_at {
            let Some(left) = grace_left(stopped_at, Instant::now()) else {
                return false;
            };
            state = wait_timeout(&self.changed, state, left);
        }
        true
    }

    /// Sleep for `pause`, or until receiving stops.
    fn pause_while_receiving(&self, pause: Duration) {
        let state = lock(&self.state);
        drop(wait_timeout_while(&self.changed, state, pause, |state| {
            state.is_receiving()
        }));
    }

    /// Open a stream, starting the accept thread if it is not running.
    fn subscribe(cspcl: &Arc<Cspcl>) -> (u64, mpsc::UnboundedReceiver<Result<Bundle>>) {
        let receiver = &cspcl.inbound;
        let mut state = wait_while(&receiver.changed, lock(&receiver.state), |state| {
            state.closing > 0
        });
        let id = state.next_id;
        state.next_id += 1;
        let (tx, mut rx) = mpsc::unbounded();

        // Claimed now, so a stream opened before the spawn below does not
        // start a second thread; the thread is spawned without the lock, so
        // listeners delivering bundles do not wait for it.
        let start_accept = !state.accept_running;
        state.accept_running = true;
        for bundle in state.pending.drain(..) {
            let _ = tx.unbounded_send(Ok(bundle));
        }
        state.subscribers.push((id, tx.clone()));
        if !state.is_receiving() {
            state.set_receiving(true);
            // Listeners waiting out the grace keep their connection.
            receiver.changed.notify_all();
        }
        drop(state);

        if start_accept {
            let spawned = thread::Builder::new()
                .name("cspcl-accept".into())
                .stack_size(RECEIVE_THREAD_STACK)
                .spawn({
                    let cspcl = Arc::clone(cspcl);
                    move || accept_loop(cspcl)
                });
            if let Err(e) = spawned {
                error!("Could not start the inbound accept thread: {e}");
                receiver.accept_failed(id, &mut rx);
                // Nothing can receive: the stream yields the error and ends.
                let _ = tx.unbounded_send(Err(Error::Spawn));
            }
        }
        (id, rx)
    }

    /// Undo a [`subscribe`](Self::subscribe) whose accept thread could not
    /// start. Its bundles are kept for a later stream; streams opened
    /// meanwhile counted on that thread, so they end too, as when it panics.
    fn accept_failed(&self, id: u64, rx: &mut mpsc::UnboundedReceiver<Result<Bundle>>) {
        let mut state = lock(&self.state);
        state.accept_running = false;
        for (sub, other) in state.subscribers.drain(..) {
            if sub != id {
                let _ = other.unbounded_send(Err(Error::Spawn));
            }
        }
        state.set_receiving(false);
        while let Ok(item) = rx.try_recv() {
            if let Ok(bundle) = item {
                state.pending.push_back(bundle);
            }
        }
        // Unlike the other paths that stop receiving, no `changed.notify_all`:
        // stopping wakes only an accept error backoff, and no accept thread
        // runs here.
    }

    /// Close a stream. Bundles left unread in it go to the newest other
    /// stream, or are kept for the next one. Their order is kept among
    /// themselves, but they may come after bundles that stream received
    /// already: bundles carry no order across streams (nor does BP promise
    /// one).
    fn unsubscribe(&self, id: u64, rx: &mut mpsc::UnboundedReceiver<Result<Bundle>>) {
        // Under the lock, so no bundle is delivered in between.
        let mut state = lock(&self.state);
        rx.close();
        state.subscribers.retain(|(sub, _)| *sub != id);
        while let Ok(item) = rx.try_recv() {
            if let Ok(bundle) = item {
                state.deliver(Ok(bundle));
            }
        }
        if state.subscribers.is_empty() && state.is_receiving() {
            debug!("Last inbound stream closed, stopping the receiver");
            state.set_receiving(false);
            // Ends an accept error backoff.
            self.changed.notify_all();
        }
    }

    /// Stop receiving and end all open streams. Until the returned guard is
    /// dropped (and those of overlapping closes), new streams wait, so the
    /// caller can close the receive socket first. The state lock is not held
    /// meanwhile: the accept thread sees that receiving stopped and leaves
    /// the accept lock to the caller.
    pub(crate) fn begin_close(&self) -> CloseGuard<'_> {
        let mut state = lock(&self.state);
        state.set_receiving(false);
        // Dropping the senders ends the streams. Bundles already in them are
        // left to their reader; `pending` is kept for the next stream, as
        // its bundles were acknowledged to their senders.
        state.subscribers.clear();
        state.closing += 1;
        // Ends an accept error backoff.
        self.changed.notify_all();
        CloseGuard { receiver: self }
    }

    fn deliver(&self, item: Result<Bundle>) {
        lock(&self.state).deliver(item);
    }
}

/// Ends a close started by [`Receiver::begin_close`].
pub(crate) struct CloseGuard<'a> {
    receiver: &'a Receiver,
}

impl Drop for CloseGuard<'_> {
    fn drop(&mut self) {
        let mut state = lock(&self.receiver.state);
        state.closing -= 1;
        if state.closing == 0 {
            self.receiver.changed.notify_all();
        }
    }
}

/// Clears `accept_running` if the accept thread exits other than by
/// deciding to stop, i.e. when it panics. The open streams are then ended,
/// so their reader notices, and the next stream starts a new thread.
struct AcceptGuard<'a> {
    receiver: &'a Receiver,
    armed: bool,
}

impl Drop for AcceptGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        error!("Inbound accept thread stopped unexpectedly, ending inbound streams");
        let mut state = lock(&self.receiver.state);
        state.accept_running = false;
        state.subscribers.clear();
        state.set_receiving(false);
    }
}

/// State shared between a listener thread and the table.
struct ListenerState {
    key: (u8, u8),
    /// Cleared once the listener has closed its connection.
    running: AtomicBool,
    /// Set to make the listener close its connection after its current read.
    evicted: AtomicBool,
    /// When the listener last received a bundle (or started), in ms since
    /// the receiver's epoch.
    last_active_ms: AtomicU64,
}

impl ListenerState {
    fn new(key: (u8, u8), now_ms: u64) -> Self {
        Self {
            key,
            running: AtomicBool::new(true),
            evicted: AtomicBool::new(false),
            last_active_ms: AtomicU64::new(now_ms),
        }
    }

    fn is_evicted(&self) -> bool {
        self.evicted.load(Ordering::Acquire)
    }
}

/// Clears `running` when dropped, and wakes an accept thread waiting for a
/// free slot.
struct RunningGuard {
    listener: Arc<ListenerState>,
    listeners: Arc<Listeners>,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        // Under the table lock, so an accept thread that just found the
        // table full is already waiting and gets the wake-up.
        let _table = lock(&self.listeners.table);
        self.listener.running.store(false, Ordering::Release);
        self.listeners.slot_freed.notify_all();
    }
}

/// The connection table and the wake-up for a slot freed in it.
#[derive(Default)]
struct Listeners {
    table: Mutex<ListenerTable>,
    slot_freed: Condvar,
}

impl Listeners {
    /// Wait up to `timeout` until another connection may be accepted;
    /// returns whether it may.
    fn wait_for_slot(&self, timeout: Duration) -> bool {
        let mut table = wait_timeout_while(&self.slot_freed, lock(&self.table), timeout, |table| {
            !table.may_accept()
        });
        table.may_accept()
    }
}

/// Inbound connections whose listener has not finished yet, including ones
/// being closed. Shared by all streams of an instance.
#[derive(Default)]
struct ListenerTable {
    listeners: Vec<Arc<ListenerState>>,
}

impl ListenerTable {
    /// Number of open connections, including ones being closed, after
    /// forgetting finished listeners.
    fn open(&mut self) -> usize {
        self.listeners
            .retain(|listener| listener.running.load(Ordering::Acquire));
        self.listeners.len()
    }

    /// Open connections that are not being closed.
    fn kept(&self) -> usize {
        self.listeners
            .iter()
            .filter(|listener| !listener.is_evicted())
            .count()
    }

    /// Whether another connection may be accepted now.
    fn may_accept(&mut self) -> bool {
        self.open() < MAX_INBOUND_CONNS + MAX_EXTRA_INBOUND_CONNS
    }

    /// Count the listener of a newly accepted connection, before its thread
    /// starts. It closes no other connection until
    /// [`supersede`](Self::supersede).
    fn register(&mut self, listener: Arc<ListenerState>) {
        self.listeners.push(listener);
    }

    /// Close the connections that a registered listener supersedes, once
    /// its thread is running. A listener that already finished (its thread
    /// did not start, or stopped at once) supersedes nothing.
    fn supersede(&mut self, listener: &Arc<ListenerState>) {
        if !listener.running.load(Ordering::Acquire) {
            return;
        }
        // libcsp gives every outgoing connection a fresh ephemeral source
        // port, so (address, port) identifies one connection, and agents
        // sharing a CSP address never collide. The same pair only reappears
        // once the peer's port counter wrapped or it restarted: the old
        // connection is then dead.
        for stale in self
            .listeners
            .iter()
            .filter(|old| old.key == listener.key && !Arc::ptr_eq(old, listener))
        {
            debug!(
                src_addr = listener.key.0,
                src_port = listener.key.1,
                "Connection reopened from the same address and port, closing the old one"
            );
            stale.evicted.store(true, Ordering::Release);
        }

        self.open();
        if self.kept() > MAX_INBOUND_CONNS {
            self.evict_least_recent_except(listener);
        }
    }

    /// Ask the least recently active listener other than `keep` to close its
    /// connection.
    fn evict_least_recent_except(&self, keep: &Arc<ListenerState>) {
        let candidate = self
            .listeners
            .iter()
            .filter(|listener| !Arc::ptr_eq(listener, keep) && !listener.is_evicted())
            .min_by_key(|listener| listener.last_active_ms.load(Ordering::Acquire));
        if let Some(listener) = candidate {
            debug!(
                src_addr = listener.key.0,
                src_port = listener.key.1,
                "More than {MAX_INBOUND_CONNS} inbound connections, closing the least recently active one"
            );
            listener.evicted.store(true, Ordering::Release);
        }
    }
}

fn accept_loop(cspcl: Arc<Cspcl>) {
    debug!("Inbound accept thread started");
    let receiver = &cspcl.inbound;
    let mut guard = AcceptGuard {
        receiver,
        armed: true,
    };

    loop {
        {
            // Decided under the lock that `subscribe` takes, so a stream
            // opened right now either sees this thread running or starts a
            // new one.
            let mut state = lock(&receiver.state);
            if !state.is_receiving() {
                state.accept_running = false;
                guard.armed = false;
                break;
            }
        }

        // The timeout makes the loop notice when receiving stops.
        if !receiver.listeners.wait_for_slot(ACCEPT_POLL) {
            continue;
        }

        let accepted = cspcl
            .poll_connection(ACCEPT_POLL)
            .and_then(|conn| spawn_listener(&cspcl, conn));
        match accepted {
            Ok(()) | Err(Error::Timeout) => {}
            Err(error) => {
                // Without the pause, peers whose connection could not be
                // served would reconnect in a tight loop.
                error!("Could not accept an inbound connection: {error}");
                receiver.deliver(Err(error));
                receiver.pause_while_receiving(ACCEPT_ERROR_BACKOFF);
            }
        }
    }

    debug!("Inbound accept thread stopped");
}

/// Start the listener thread of an accepted connection. On failure the
/// connection is closed and [`Error::Spawn`] returned.
fn spawn_listener(cspcl: &Arc<Cspcl>, conn: cspcl_sys::types::AcceptedConn) -> Result<()> {
    let key = (conn.src_addr, conn.src_port);
    debug!("New connection made with {}:{}", key.0, key.1);

    let state = Arc::new(ListenerState::new(key, cspcl.inbound.now_ms()));
    // Counted from now on, so the accept thread waits for its slot. Should
    // the thread not start, dropping the task marks it finished, and it has
    // closed no other connection.
    lock(&cspcl.inbound.listeners.table).register(Arc::clone(&state));
    let task = ListenerTask {
        conn: OwnedConn {
            conn,
            cspcl: Arc::clone(cspcl),
        },
        _running: RunningGuard {
            listener: Arc::clone(&state),
            listeners: Arc::clone(&cspcl.inbound.listeners),
        },
    };
    let spawned = thread::Builder::new()
        .name(format!("cspcl-rx-{}:{}", key.0, key.1))
        .stack_size(RECEIVE_THREAD_STACK)
        .spawn({
            let state = Arc::clone(&state);
            move || listen(task, &state)
        });
    match spawned {
        Ok(_) => {
            lock(&cspcl.inbound.listeners.table).supersede(&state);
            Ok(())
        }
        Err(e) => {
            // The task was dropped with the closure: the connection is closed.
            error!("Could not start a listener for {}:{}: {e}", key.0, key.1);
            Err(Error::Spawn)
        }
    }
}

/// An accepted connection together with the instance it belongs to.
///
/// Fields drop in declaration order, so the connection is always closed
/// before the instance can be cleaned up.
struct OwnedConn {
    conn: cspcl_sys::types::AcceptedConn,
    cspcl: Arc<Cspcl>,
}

/// Everything a listener thread owns. Fields drop in declaration order: the
/// connection is closed (and the instance released) before the listener is
/// marked as finished, also if the thread never starts.
struct ListenerTask {
    conn: OwnedConn,
    _running: RunningGuard,
}

fn listen(task: ListenerTask, state: &ListenerState) {
    let conn = &task.conn.conn;
    let receiver = &task.conn.cspcl.inbound;
    debug!(
        src_addr = conn.src_addr,
        src_port = conn.src_port,
        "Inbound connection listener started"
    );

    // Reused by every read of this connection.
    let mut buffer = vec![0; cspcl_sys::CSPCL_MAX_BUNDLE_SIZE as usize];
    let read_timeout = expected_read_timeout();
    loop {
        // While no stream is open, start no new read (it would acknowledge a
        // bundle nobody takes), and close the connection unless a stream is
        // opened again within the grace. An eviction during the grace is
        // seen once it ends.
        if !receiver.wait_while_stopped() || state.is_evicted() {
            break;
        }

        let read_started = Instant::now();
        match recv_bundle_from_conn(conn, &mut buffer) {
            Ok(bundle) => {
                state
                    .last_active_ms
                    .store(receiver.now_ms(), Ordering::Release);
                debug!(
                    "Received bundle from {}:{}",
                    bundle.src_addr, bundle.src_port
                );
                receiver.deliver(Ok(bundle));
            }
            Err(Error::Timeout) if woken_by_reset(read_started.elapsed(), read_timeout) => {
                debug!(
                    src_addr = conn.src_addr,
                    src_port = conn.src_port,
                    "Inbound connection reset by its peer"
                );
                break;
            }
            Err(Error::Timeout) => {
                let idle_since = state.last_active_ms.load(Ordering::Acquire);
                if idle_expired(idle_since, receiver.now_ms()) {
                    debug!(
                        src_addr = conn.src_addr,
                        src_port = conn.src_port,
                        "Inbound connection idle for {INBOUND_IDLE_TIMEOUT:?}, closing it"
                    );
                    break;
                }
                trace!("Timed out while waiting for new bundle");
            }
            Err(error) => {
                warn!("Got error while trying to receive bundle: {error}");
                receiver.deliver(Err(error));
                break;
            }
        }
    }

    debug!(
        src_addr = conn.src_addr,
        src_port = conn.src_port,
        "Inbound connection listener stopped"
    );
    // Dropping `task` closes the connection, releases the instance, then
    // marks the listener as finished.
}

/// How much of the grace period started at `stopped_at` is left at `now`,
/// `None` once it is over.
fn grace_left(stopped_at: Instant, now: Instant) -> Option<Duration> {
    STOP_GRACE
        .checked_sub(now.saturating_duration_since(stopped_at))
        .filter(|left| !left.is_zero())
}

/// Whether a connection last active at `last_active_ms` has been idle for
/// too long at `now_ms`.
fn idle_expired(last_active_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_active_ms) >= INBOUND_IDLE_TIMEOUT.as_millis() as u64
}

fn recv_bundle_from_conn(
    conn: &cspcl_sys::types::AcceptedConn,
    buffer: &mut [u8],
) -> Result<Bundle> {
    let len =
        cspcl_sys::types::recv_bundle_from_conn_into(conn, buffer).map_err(Error::from_raw)?;

    Ok(Bundle {
        data: buffer[..len].to_vec(),
        src_addr: conn.src_addr,
        src_port: conn.src_port,
    })
}

/// A stream of inbound bundles, see [`Cspcl::inbound`].
pub struct InboundStream {
    rx: mpsc::UnboundedReceiver<Result<Bundle>>,
    id: u64,
    cspcl: Arc<Cspcl>,
}

impl InboundStream {
    pub fn new(cspcl: Arc<Cspcl>) -> Self {
        debug!("Creating inbound stream");
        let (id, rx) = Receiver::subscribe(&cspcl);
        Self { rx, id, cspcl }
    }
}

impl Drop for InboundStream {
    fn drop(&mut self) {
        debug!("Closing inbound stream");
        self.cspcl.inbound.unsubscribe(self.id, &mut self.rx);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn admit(table: &mut ListenerTable, key: (u8, u8)) -> Arc<ListenerState> {
        let listener = Arc::new(ListenerState::new(key, 0));
        table.register(Arc::clone(&listener));
        table.supersede(&listener);
        listener
    }

    fn finish(listener: &ListenerState) {
        listener.running.store(false, Ordering::Release);
    }

    fn full_table() -> (ListenerTable, Vec<Arc<ListenerState>>) {
        let mut table = ListenerTable::default();
        let admitted = (0..MAX_INBOUND_CONNS as u8)
            .map(|port| admit(&mut table, (5, port)))
            .collect();
        (table, admitted)
    }

    fn bundle(data: &[u8]) -> Bundle {
        Bundle {
            data: data.to_vec(),
            src_addr: 5,
            src_port: 20,
        }
    }

    #[test]
    fn same_address_different_ports_coexist() {
        let mut table = ListenerTable::default();
        let first = admit(&mut table, (5, 20));
        let second = admit(&mut table, (5, 21));

        assert!(!first.is_evicted());
        assert!(!second.is_evicted());
        assert_eq!(table.open(), 2);
    }

    #[test]
    fn reopened_connection_evicts_the_old_one() {
        let mut table = ListenerTable::default();
        let old = admit(&mut table, (5, 20));
        let new = admit(&mut table, (5, 20));

        assert!(old.is_evicted());
        assert!(!new.is_evicted());
        // The old connection counts until its listener has closed it.
        assert_eq!(table.open(), 2);
        finish(&old);
        assert_eq!(table.open(), 1);
    }

    #[test]
    fn reconnect_into_a_full_table_closes_only_the_stale_connection() {
        let (mut table, admitted) = full_table();

        let reopened = admit(&mut table, (5, 3));

        assert!(admitted[3].is_evicted());
        assert!(
            admitted
                .iter()
                .enumerate()
                .all(|(i, listener)| i == 3 || !listener.is_evicted()),
            "a healthy connection was closed as well"
        );
        assert!(!reopened.is_evicted());
    }

    #[test]
    fn a_listener_that_did_not_start_closes_no_connection() {
        let (mut table, admitted) = full_table();
        let failed = Arc::new(ListenerState::new((5, 3), 0));
        table.register(Arc::clone(&failed));

        // Its thread could not be spawned (or stopped at once).
        finish(&failed);
        table.supersede(&failed);

        assert!(
            admitted.iter().all(|listener| !listener.is_evicted()),
            "a healthy connection was closed for a listener that never ran"
        );
        assert_eq!(table.open(), MAX_INBOUND_CONNS);
    }

    #[test]
    fn full_table_keeps_all_connections_until_another_arrives() {
        let (mut table, admitted) = full_table();

        assert!(admitted.iter().all(|listener| !listener.is_evicted()));
        assert!(table.may_accept(), "a further connection must be accepted");
    }

    #[test]
    fn connection_beyond_the_limit_evicts_least_recently_active() {
        let (mut table, admitted) = full_table();
        for listener in &admitted[1..] {
            listener.last_active_ms.store(1_000, Ordering::Release);
        }

        let newest = admit(&mut table, (6, 0));

        assert!(admitted[0].is_evicted());
        assert!(admitted[1..].iter().all(|listener| !listener.is_evicted()));
        assert!(!newest.is_evicted());
        // The evicted connection counts until it is closed.
        assert_eq!(table.open(), MAX_INBOUND_CONNS + 1);
        finish(&admitted[0]);
        assert_eq!(table.open(), MAX_INBOUND_CONNS);
    }

    #[test]
    fn new_connections_are_accepted_while_evictions_finish_up_to_the_slack() {
        let (mut table, _admitted) = full_table();
        let mut newcomers = Vec::new();
        for port in 0..MAX_EXTRA_INBOUND_CONNS as u8 {
            assert!(
                table.may_accept(),
                "newcomer {port} had to wait for an eviction"
            );
            newcomers.push(admit(&mut table, (6, port)));
        }

        assert_eq!(table.open(), MAX_INBOUND_CONNS + MAX_EXTRA_INBOUND_CONNS);
        assert!(!table.may_accept(), "accepted beyond the slack");
        assert!(newcomers.iter().all(|listener| !listener.is_evicted()));
    }

    #[test]
    fn running_guard_marks_listener_finished() {
        let listeners = Arc::new(Listeners::default());
        let listener = admit(&mut lock(&listeners.table), (5, 20));
        drop(RunningGuard {
            listener,
            listeners: Arc::clone(&listeners),
        });

        assert_eq!(lock(&listeners.table).open(), 0);
    }

    #[test]
    fn a_finished_listener_wakes_the_accept_thread_waiting_for_a_slot() {
        let listeners = Arc::new(Listeners::default());
        let admitted: Vec<_> = (0..(MAX_INBOUND_CONNS + MAX_EXTRA_INBOUND_CONNS) as u8)
            .map(|port| admit(&mut lock(&listeners.table), (5, port)))
            .collect();
        assert!(!listeners.wait_for_slot(Duration::ZERO), "table not full");

        let started = Instant::now();
        let accepted = thread::scope(|s| {
            let waiter = s.spawn(|| listeners.wait_for_slot(Duration::from_secs(5)));
            thread::sleep(Duration::from_millis(50));
            drop(RunningGuard {
                listener: Arc::clone(&admitted[0]),
                listeners: Arc::clone(&listeners),
            });
            waiter.join().unwrap()
        });

        assert!(accepted);
        assert!(started.elapsed() < Duration::from_secs(2), "not woken");
    }

    #[test]
    fn early_wake_ups_are_resets_full_timeouts_are_not() {
        let expected = Duration::from_secs(10);
        assert!(woken_by_reset(Duration::from_millis(5), expected));
        // A reset late in the read: missed by a check against a fixed
        // fraction of the requested timeout.
        assert!(woken_by_reset(Duration::from_secs(8), expected));
        assert!(!woken_by_reset(expected, expected));
        assert!(!woken_by_reset(Duration::from_millis(10_200), expected));
    }

    #[test]
    fn panicking_accept_thread_resets_state_and_ends_streams() {
        let receiver = Receiver::new();
        let (tx, mut rx) = mpsc::unbounded();
        {
            let mut state = lock(&receiver.state);
            state.accept_running = true;
            state.subscribers.push((0, tx));
            state.set_receiving(true);
        }

        let panicked = thread::scope(|s| {
            s.spawn(|| {
                let _guard = AcceptGuard {
                    receiver: &receiver,
                    armed: true,
                };
                panic!("accept loop failed");
            })
            .join()
        });

        assert!(panicked.is_err());
        let state = lock(&receiver.state);
        assert!(
            !state.accept_running,
            "no accept thread would ever start again"
        );
        assert!(!state.is_receiving());
        assert!(
            rx.try_recv().is_err_and(|e| e.is_closed()),
            "stream did not end"
        );
    }

    #[test]
    fn failed_accept_spawn_keeps_bundles_and_ends_streams() {
        let receiver = Receiver::new();
        let (own_tx, mut own_rx) = mpsc::unbounded();
        let (other_tx, mut other_rx) = mpsc::unbounded();
        {
            let mut state = lock(&receiver.state);
            state.accept_running = true;
            state.subscribers.push((0, other_tx));
            state.subscribers.push((1, own_tx.clone()));
            state.set_receiving(true);
        }
        let bundle = Bundle {
            data: vec![1],
            src_addr: 2,
            src_port: 3,
        };
        own_tx.unbounded_send(Ok(bundle)).unwrap();

        receiver.accept_failed(1, &mut own_rx);

        let state = lock(&receiver.state);
        assert!(!state.accept_running, "no accept thread would ever start");
        assert!(!state.is_receiving());
        assert!(state.subscribers.is_empty());
        assert_eq!(state.pending.len(), 1, "bundle not kept for a later stream");
        assert!(matches!(other_rx.try_recv(), Ok(Err(Error::Spawn))));
        assert!(
            other_rx.try_recv().is_err_and(|e| e.is_closed()),
            "other stream did not end"
        );
    }

    #[test]
    fn grace_lasts_stop_grace() {
        let stopped = Instant::now();
        assert_eq!(grace_left(stopped, stopped), Some(STOP_GRACE));
        assert!(grace_left(stopped, stopped + STOP_GRACE / 2).is_some());
        assert_eq!(grace_left(stopped, stopped + STOP_GRACE), None);
    }

    #[test]
    fn grace_starts_when_receiving_stops_not_again() {
        let mut state = ReceiverState::new();
        state.set_receiving(true);
        assert!(state.is_receiving());
        state.set_receiving(false);
        let stopped = state.stopped_at;
        assert!(stopped.is_some());

        // Already off: e.g. close_rx_socket after the last stream closed.
        thread::sleep(Duration::from_millis(5));
        state.set_receiving(false);

        assert_eq!(state.stopped_at, stopped);
    }

    #[test]
    fn listener_keeps_its_connection_when_receiving_resumes_within_the_grace() {
        let receiver = Receiver::new();
        lock(&receiver.state).set_receiving(false);

        let kept = thread::scope(|s| {
            let waiter = s.spawn(|| receiver.wait_while_stopped());
            thread::sleep(Duration::from_millis(50));
            lock(&receiver.state).set_receiving(true);
            receiver.changed.notify_all();
            waiter.join().unwrap()
        });

        assert!(kept);
    }

    #[test]
    fn listener_closes_its_connection_after_the_grace() {
        let receiver = Receiver::new();
        lock(&receiver.state).stopped_at = Some(Instant::now() - STOP_GRACE);

        assert!(!receiver.wait_while_stopped());
    }

    /// Runs `stop` while the accept thread would be backing off after an
    /// error, and checks that the backoff ends early.
    fn assert_stop_ends_the_accept_backoff(stop: impl FnOnce(&Receiver)) {
        let receiver = Receiver::new();
        let (tx, _rx) = mpsc::unbounded();
        {
            let mut state = lock(&receiver.state);
            state.subscribers.push((0, tx));
            state.set_receiving(true);
        }

        let started = Instant::now();
        thread::scope(|s| {
            let backoff = s.spawn(|| receiver.pause_while_receiving(Duration::from_secs(5)));
            thread::sleep(Duration::from_millis(50));
            stop(&receiver);
            backoff.join().unwrap();
        });

        assert!(started.elapsed() < Duration::from_secs(2), "backoff ran on");
    }

    #[test]
    fn closing_the_last_stream_ends_the_accept_backoff() {
        assert_stop_ends_the_accept_backoff(|receiver| {
            let (_, mut rx) = mpsc::unbounded::<Result<Bundle>>();
            receiver.unsubscribe(0, &mut rx);
        });
    }

    #[test]
    fn close_rx_socket_ends_the_accept_backoff() {
        assert_stop_ends_the_accept_backoff(|receiver| drop(receiver.begin_close()));
    }

    #[test]
    fn overlapping_closes_hold_new_streams_until_the_last_one_ends() {
        let receiver = Receiver::new();
        let first = receiver.begin_close();
        let second = receiver.begin_close();

        drop(first);
        assert_eq!(lock(&receiver.state).closing, 1, "second close still runs");
        drop(second);
        assert_eq!(lock(&receiver.state).closing, 0);
    }

    #[test]
    fn idle_close_after_the_idle_timeout() {
        let timeout = INBOUND_IDLE_TIMEOUT.as_millis() as u64;
        assert!(!idle_expired(1_000, 1_000 + timeout - 1));
        assert!(idle_expired(1_000, 1_000 + timeout));
        // A clock reading before the last activity is never idle.
        assert!(!idle_expired(1_000, 0));
    }

    #[test]
    fn bundles_go_to_the_newest_stream() {
        let mut state = ReceiverState::new();
        let (old_tx, mut old_rx) = mpsc::unbounded();
        let (new_tx, mut new_rx) = mpsc::unbounded();
        state.subscribers.push((0, old_tx));
        state.subscribers.push((1, new_tx));

        state.deliver(Ok(bundle(b"x")));

        assert_eq!(new_rx.try_recv().unwrap().unwrap().data, b"x");
        assert!(old_rx.try_recv().is_err());
    }

    #[test]
    fn bundles_are_kept_while_no_stream_is_open() {
        let mut state = ReceiverState::new();

        state.deliver(Ok(bundle(b"x")));
        state.deliver(Err(Error::Timeout));

        assert_eq!(
            state
                .pending
                .iter()
                .map(|b| b.data.clone())
                .collect::<Vec<_>>(),
            [b"x".to_vec()],
            "the bundle is kept, the error dropped"
        );
    }

    /// The only test in this binary that initializes libcsp.
    #[test]
    fn peer_reset_closes_the_inbound_connection() {
        use crate::{CspAddress, Interface};
        use futures::StreamExt;

        const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };
        let wait_for_open = |cspcl: &Cspcl, expected: usize, what: &str| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while lock(&cspcl.inbound.listeners.table).open() != expected {
                assert!(Instant::now() < deadline, "{what}");
                thread::sleep(Duration::from_millis(20));
            }
        };

        let receiver = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
        let mut inbound = Arc::clone(&receiver).inbound();

        // A second instance on the same node has its own connection pool.
        let peer = Cspcl::new(LOCAL, Interface::Loopback).expect("peer instance");
        peer.send_bundle(b"hello", LOCAL).expect("send");
        let received = futures::executor::block_on(inbound.next())
            .expect("stream open")
            .expect("valid bundle");
        assert_eq!(received.data, b"hello");
        wait_for_open(&receiver, 1, "inbound connection not registered");

        // Closing the peer's pooled connection resets ours, right after the
        // listener started its next read.
        drop(peer);
        wait_for_open(&receiver, 0, "reset inbound connection was kept open");

        // A reset late in a read, past the old fixed 2.5 s check.
        let peer = Cspcl::new(LOCAL, Interface::Loopback).expect("peer instance");
        peer.send_bundle(b"again", LOCAL).expect("send");
        let received = futures::executor::block_on(inbound.next())
            .expect("stream open")
            .expect("valid bundle");
        assert_eq!(received.data, b"again");
        wait_for_open(&receiver, 1, "second inbound connection not registered");
        thread::sleep(Duration::from_secs(6));
        drop(peer);
        wait_for_open(&receiver, 0, "late reset inbound connection was kept open");
    }
}
