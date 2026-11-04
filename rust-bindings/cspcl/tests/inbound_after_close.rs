//! `close_rx_socket` ends the current streams; a later stream receives again
//! on the same instance.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stream_created_after_close_receives() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
    let mut before = Arc::clone(&cspcl).inbound();

    cspcl.close_rx_socket();
    let ended = tokio::time::timeout(Duration::from_secs(1), before.next()).await;
    assert!(
        matches!(ended, Ok(None)),
        "stream open before close_rx_socket did not end"
    );

    let mut after = Arc::clone(&cspcl).inbound();
    cspcl
        .send_bundle_async(b"hello again".to_vec(), LOCAL)
        .await
        .expect("send after close and reopen");
    let received = tokio::time::timeout(Duration::from_secs(5), after.next())
        .await
        .expect("bundle within 5 s")
        .expect("stream open")
        .expect("valid bundle");
    assert_eq!(received.data, b"hello again");
}
