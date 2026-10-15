//! A send cancelled while queued behind a slow send must not go out.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };
/// No such node exists on the loopback interface.
const UNREACHABLE: CspAddress = CspAddress { addr: 9, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_cancelled_behind_a_slow_send_is_skipped() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
    let mut inbound = Arc::clone(&cspcl).inbound();

    // Occupies the instance's sends for one connect timeout.
    let slow = tokio::spawn({
        let cspcl = Arc::clone(&cspcl);
        async move { cspcl.send_bundle_async(b"lost".to_vec(), UNREACHABLE).await }
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !slow.is_finished(),
        "slow send finished too fast to queue behind it"
    );

    // Has a blocking thread, but must wait for the slow send.
    let queued = tokio::time::timeout(
        Duration::from_millis(200),
        cspcl.send_bundle_async(b"must not be sent".to_vec(), LOCAL),
    )
    .await;
    assert!(
        queued.is_err(),
        "send should still be queued when cancelled"
    );

    assert!(slow.await.expect("slow send task").is_err());

    // Had the queued send run, the loopback bundle would arrive right after
    // the slow send finished.
    let received = tokio::time::timeout(Duration::from_secs(3), inbound.next()).await;
    assert!(
        received.is_err(),
        "cancelled send was delivered: {:?}",
        received.map(|b| b.map(|b| b.map(|b| b.data)))
    );
}
