//! A `send_bundle_async` future dropped before its send started must not send.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };
const BLOCKING_THREADS: usize = 2;

#[test]
fn dropped_send_that_has_not_started_is_skipped() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(BLOCKING_THREADS)
        .enable_all()
        .build()
        .expect("runtime");

    runtime.block_on(async {
        let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
        let mut inbound = Arc::clone(&cspcl).inbound();

        // Occupy every blocking thread so the send below stays queued.
        let blockers: Vec<_> = (0..BLOCKING_THREADS)
            .map(|_| tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(500))))
            .collect();

        let queued = tokio::time::timeout(
            Duration::from_millis(100),
            cspcl.send_bundle_async(b"must not be sent".to_vec(), LOCAL),
        )
        .await;
        assert!(
            queued.is_err(),
            "send should still be queued when cancelled"
        );

        for blocker in blockers {
            blocker.await.expect("blocker");
        }

        // Had the send run, the loopback bundle would arrive within a few
        // hundred milliseconds once blocking threads are free.
        let received = tokio::time::timeout(Duration::from_secs(2), inbound.next()).await;
        assert!(
            received.is_err(),
            "cancelled send was delivered: {:?}",
            received.map(|b| b.map(|b| b.map(|b| b.data)))
        );
    });
}
