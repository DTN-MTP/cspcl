//! Bundles acknowledged to their sender must not be lost when the stream they
//! were meant for is dropped unread: the next stream gets them, in order.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unread_bundles_of_a_dropped_stream_reach_the_next_stream() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));

    // Both sends return once the receiver has acknowledged the bundle, so
    // both are owed to the application whichever stream is open.
    let unread = Arc::clone(&cspcl).inbound();
    for data in [&b"first"[..], b"second"] {
        cspcl
            .send_bundle_async(data.to_vec(), LOCAL)
            .await
            .expect("send acknowledged");
    }
    drop(unread);

    let mut next = Arc::clone(&cspcl).inbound();
    for expected in [&b"first"[..], b"second"] {
        let received = tokio::time::timeout(Duration::from_secs(5), next.next())
            .await
            .expect("acknowledged bundle was lost")
            .expect("stream open")
            .expect("valid bundle");
        assert_eq!(received.data, expected);
    }
}
