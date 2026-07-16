use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

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
