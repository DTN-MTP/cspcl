//! `close_rx_socket` must not wait for sends in progress.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cspcl::{CspAddress, Cspcl, Interface};

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };
/// No such node exists on the loopback interface.
const UNREACHABLE: CspAddress = CspAddress { addr: 9, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn close_rx_socket_does_not_wait_for_sends() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
    let _inbound = Arc::clone(&cspcl).inbound();

    let sends: Vec<_> = (0..3)
        .map(|_| {
            let cspcl = Arc::clone(&cspcl);
            tokio::spawn(
                async move { cspcl.send_bundle_async(b"lost".to_vec(), UNREACHABLE).await },
            )
        })
        .collect();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        sends.iter().all(|send| !send.is_finished()),
        "sends to an unreachable peer finished too fast to test anything"
    );

    let start = Instant::now();
    cspcl.close_rx_socket();
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "close_rx_socket waited {elapsed:?} for sends in progress"
    );
    for send in sends {
        assert!(send.await.expect("send task").is_err());
    }
}
