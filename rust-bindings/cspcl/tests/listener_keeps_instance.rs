//! Inbound listeners must keep the instance alive until their connection is
//! closed, so an instance is never cleaned up under an open connection.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn listener_keeps_instance_alive_until_connection_closed() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
    let mut inbound = Arc::clone(&cspcl).inbound();

    // Open an inbound connection (to ourselves) so that a listener runs.
    cspcl
        .send_bundle_async(b"hello".to_vec(), LOCAL)
        .await
        .expect("loopback send");
    tokio::time::timeout(Duration::from_secs(5), inbound.next())
        .await
        .expect("bundle within 5 s")
        .expect("stream open")
        .expect("valid bundle");

    let weak = Arc::downgrade(&cspcl);
    drop(inbound);
    drop(cspcl);
    // Let the aborted accept loop release its reference.
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(
        weak.upgrade().is_some(),
        "instance was dropped while a listener still held its connection"
    );

    // The listener notices the shutdown after its current receive times out,
    // closes the connection, then releases the instance. On RDP connections
    // libcsp raises the receive timeout to the RDP connection timeout (10 s
    // by default), so allow well beyond that.
    let deadline = Instant::now() + Duration::from_secs(30);
    while weak.upgrade().is_some() {
        assert!(
            Instant::now() < deadline,
            "listener never released the instance"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
