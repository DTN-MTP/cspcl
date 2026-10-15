//! Dropping one inbound stream must not stop streams created later.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_stream_receives_after_an_earlier_stream_was_dropped() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));
    drop(Arc::clone(&cspcl).inbound());

    let mut inbound = Arc::clone(&cspcl).inbound();
    cspcl
        .send_bundle_async(b"hello".to_vec(), LOCAL)
        .await
        .expect("loopback send");
    let bundle = tokio::time::timeout(Duration::from_secs(5), inbound.next())
        .await
        .expect("bundle within 5 s")
        .expect("stream open")
        .expect("valid bundle");
    assert_eq!(bundle.data, b"hello");
}
