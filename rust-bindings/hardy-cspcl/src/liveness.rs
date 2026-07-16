use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use hardy_async::CancellationToken;
use hardy_bpa::cla::{self, ClaAddress};
use hardy_bpv7::eid::NodeId;
use tracing::{info, warn};

/// Per-peer liveness state. Lock-free; safe under concurrent `forward()` calls.
pub struct PeerLiveness {
    consecutive_failures: AtomicU32,
    recovering: AtomicBool,
}

impl PeerLiveness {
    pub fn new() -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            recovering: AtomicBool::new(false),
        }
    }

    /// A send succeeded: clear the failure streak. Leaves `recovering` untouched.
    pub fn on_send_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
    }

    /// A send failed. Returns `true` exactly once — on the call that both
    /// reaches `threshold` and wins the race to flip `recovering` false->true.
    /// The winner is responsible for starting the recovery task.
    pub fn on_send_failure(&self, threshold: u32) -> bool {
        let n = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
        if n >= threshold {
            self.recovering
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        } else {
            false
        }
    }

    /// Recovery finished: return to the Up state.
    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.recovering.store(false, Ordering::Release);
    }

    pub fn is_recovering(&self) -> bool {
        self.recovering.load(Ordering::Acquire)
    }
}

impl Default for PeerLiveness {
    fn default() -> Self {
        Self::new()
    }
}

/// Drive one peer's down->up recovery: drop the route, probe until the node
/// answers, then re-add the route (which triggers hardy's poll_waiting re-drive
/// of parked Waiting bundles). Runs on its own task; honors `cancel` for
/// shutdown. `probe` returns `true` once the peer is reachable.
pub async fn run_recovery<P, Fut>(
    sink: Arc<dyn cla::Sink>,
    cla_addr: ClaAddress,
    node_ids: Vec<NodeId>,
    liveness: Arc<PeerLiveness>,
    heartbeat: Duration,
    cancel: CancellationToken,
    probe: P,
) where
    P: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = bool> + Send,
{
    // 1. Drop the route: hardy resets this peer's ForwardPending -> Waiting.
    if let Err(e) = sink.remove_peer(&cla_addr).await {
        warn!("remove_peer failed for {cla_addr}: {e}");
    }

    // 2. Probe until the node answers, or we are cancelled.
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Recovery for {cla_addr} cancelled; leaving bundles Waiting");
                return;
            }
            _ = tokio::time::sleep(heartbeat) => {}
        }
        if probe().await {
            break;
        }
    }

    // 3. Re-add the route -> hardy notify_updated -> poll_waiting re-drives.
    //    Bounded retries: a false/Err here would strand bundles, so don't
    //    silently no-op.
    let mut attempts = 0u32;
    loop {
        match sink.add_peer(cla_addr.clone(), &node_ids).await {
            Ok(_) => break,
            Err(e) => {
                attempts += 1;
                warn!("add_peer failed (attempt {attempts}) for {cla_addr}: {e}");
                if attempts >= 3 {
                    break;
                }
                tokio::time::sleep(heartbeat).await;
            }
        }
    }

    // 4. Back to Up.
    liveness.reset();
    info!("Peer {cla_addr} recovered; route re-added, Waiting bundles re-driven");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn stays_up_below_threshold() {
        let p = PeerLiveness::new();
        assert!(!p.on_send_failure(3)); // 1
        assert!(!p.on_send_failure(3)); // 2
        assert!(!p.is_recovering());
    }

    #[test]
    fn crosses_threshold_exactly_once() {
        let p = PeerLiveness::new();
        assert!(!p.on_send_failure(3)); // 1
        assert!(!p.on_send_failure(3)); // 2
        assert!(p.on_send_failure(3)); //  3 -> true, wins recovery
        assert!(p.is_recovering());
        // Already recovering: further failures never re-trigger.
        assert!(!p.on_send_failure(3));
    }

    #[test]
    fn success_resets_failure_count() {
        let p = PeerLiveness::new();
        p.on_send_failure(3);
        p.on_send_failure(3);
        p.on_send_success();
        assert!(!p.on_send_failure(3)); // count restarted at 1
        assert!(!p.is_recovering());
    }

    #[test]
    fn reset_returns_to_up() {
        let p = PeerLiveness::new();
        assert!(p.on_send_failure(1)); // threshold 1 -> immediate
        assert!(p.is_recovering());
        p.reset();
        assert!(!p.is_recovering());
        // Fresh cycle possible after reset.
        assert!(p.on_send_failure(1));
    }

    #[test]
    fn only_one_thread_wins_the_threshold() {
        let p = Arc::new(PeerLiveness::new());
        // Prime to threshold-1 so the next failure crosses.
        p.on_send_failure(2);
        let wins = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let p = p.clone();
            let wins = wins.clone();
            handles.push(std::thread::spawn(move || {
                if p.on_send_failure(2) {
                    wins.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(wins.load(Ordering::SeqCst), 1);
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use bytes::Bytes;
    use hardy_async::async_trait;
    use hardy_bpa::cla::{self, ClaAddress};

    #[derive(Default)]
    struct MockSink {
        removed: AtomicU32,
        added: AtomicU32,
    }

    #[async_trait]
    impl cla::Sink for MockSink {
        async fn unregister(&self) {}
        async fn dispatch(
            &self,
            _bundle: Bytes,
            _peer_node: Option<&hardy_bpv7::eid::NodeId>,
            _peer_addr: Option<&ClaAddress>,
        ) -> cla::Result<()> {
            Ok(())
        }
        async fn add_peer(
            &self,
            _cla_addr: ClaAddress,
            _node_ids: &[hardy_bpv7::eid::NodeId],
        ) -> cla::Result<bool> {
            self.added.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
        async fn remove_peer(&self, _cla_addr: &ClaAddress) -> cla::Result<bool> {
            self.removed.fetch_add(1, Ordering::SeqCst);
            Ok(true)
        }
    }

    #[tokio::test]
    async fn recovery_removes_probes_then_readds_and_resets() {
        let sink = Arc::new(MockSink::default());
        let liveness = Arc::new(PeerLiveness::new());
        // Simulate forward() having claimed recovery.
        assert!(liveness.on_send_failure(1));
        assert!(liveness.is_recovering());

        let cla_addr = ClaAddress::Private(Bytes::from_static(&[2, 0]));
        let node_ids = vec!["ipn:2.0".parse().unwrap()];
        let cancel = hardy_async::CancellationToken::new();

        // probe: false twice, then true (peer recovers on 3rd attempt).
        let calls = Arc::new(AtomicU32::new(0));
        let calls_probe = calls.clone();
        let probe = move || {
            let calls = calls_probe.clone();
            async move { calls.fetch_add(1, Ordering::SeqCst) >= 2 }
        };

        run_recovery(
            sink.clone(),
            cla_addr,
            node_ids,
            liveness.clone(),
            Duration::from_millis(1),
            cancel,
            probe,
        )
        .await;

        assert_eq!(sink.removed.load(Ordering::SeqCst), 1);
        assert_eq!(sink.added.load(Ordering::SeqCst), 1);
        assert!(calls.load(Ordering::SeqCst) >= 3);
        assert!(!liveness.is_recovering());
    }

    #[tokio::test]
    async fn recovery_cancelled_does_not_readd() {
        let sink = Arc::new(MockSink::default());
        let liveness = Arc::new(PeerLiveness::new());
        assert!(liveness.on_send_failure(1));
        let cla_addr = ClaAddress::Private(Bytes::from_static(&[2, 0]));
        let node_ids = vec!["ipn:2.0".parse().unwrap()];
        let cancel = hardy_async::CancellationToken::new();
        cancel.cancel(); // cancelled before probing starts

        run_recovery(
            sink.clone(),
            cla_addr,
            node_ids,
            liveness.clone(),
            Duration::from_millis(1),
            cancel,
            move || async { false }, // never succeeds
        )
        .await;

        assert_eq!(sink.removed.load(Ordering::SeqCst), 1);
        assert_eq!(sink.added.load(Ordering::SeqCst), 0); // never re-added
    }
}
