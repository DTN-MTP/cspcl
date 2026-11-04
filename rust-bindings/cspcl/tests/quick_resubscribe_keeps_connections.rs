//! Closing a stream and opening a new one within the 1 s grace must keep the
//! existing inbound connections: a later bundle arrives on the same
//! connection.
//!
//! Runs in its own test binary because libcsp is a process-wide singleton.

use std::sync::Arc;
use std::time::Duration;

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };

async fn send(cspcl: &Arc<Cspcl>, data: &[u8]) {
    cspcl
        .send_bundle_async(data.to_vec(), LOCAL)
        .await
        .expect("loopback send");
}

async fn receive(inbound: &mut cspcl::InboundStream) -> cspcl::Bundle {
    tokio::time::timeout(Duration::from_secs(5), inbound.next())
        .await
        .expect("bundle within 5 s")
        .expect("stream open")
        .expect("valid bundle")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quick_resubscribe_keeps_inbound_connections() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));

    let mut first = Arc::clone(&cspcl).inbound();
    send(&cspcl, b"before").await;
    let before = receive(&mut first).await;
    drop(first);

    // Completes the read that was in progress when the stream closed: the
    // listener then waits without reading, and closes the connection unless
    // a stream is opened within the grace.
    send(&cspcl, b"between").await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let mut second = Arc::clone(&cspcl).inbound();
    assert_eq!(receive(&mut second).await.data, b"between");
    // Past the end of the grace: the listener must have resumed, not merely
    // not closed yet.
    tokio::time::sleep(Duration::from_secs(1)).await;

    send(&cspcl, b"after").await;
    let after = receive(&mut second).await;
    assert_eq!(after.data, b"after");
    assert_eq!(
        (after.src_addr, after.src_port),
        (before.src_addr, before.src_port),
        "the inbound connection was torn down and the sender had to reconnect"
    );
}
