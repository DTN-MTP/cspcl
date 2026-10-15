//! Integration tests for `Cspcl::send_bundle_async` over the loopback interface.
//!
//! libcsp is a process-wide singleton, so all scenarios share one `Cspcl`
//! instance and run sequentially in a single test.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::time::{Duration, Instant};

use cspcl::{CspAddress, Cspcl, Interface};
use futures::StreamExt;

const LOCAL: CspAddress = CspAddress { addr: 1, port: 10 };
/// No such node exists on the loopback interface.
const UNREACHABLE: CspAddress = CspAddress { addr: 9, port: 10 };

#[tokio::test(flavor = "current_thread")]
async fn send_bundle_async_over_loopback() {
    let cspcl = Arc::new(Cspcl::new(LOCAL, Interface::Loopback).expect("init cspcl"));

    // Round trip to ourselves.
    let mut inbound = Arc::clone(&cspcl).inbound();
    let bundle = b"hello picod3tn".to_vec();
    cspcl
        .send_bundle_async(bundle.clone(), LOCAL)
        .await
        .expect("loopback send");
    let received = tokio::time::timeout(Duration::from_secs(5), inbound.next())
        .await
        .expect("bundle within 5 s")
        .expect("stream open")
        .expect("valid bundle");
    assert_eq!(received.data, bundle);
    assert_eq!(received.src_addr, LOCAL.addr);

    // A failing send must not stall the (single-threaded) runtime.
    let ticks = Arc::new(AtomicU32::new(0));
    let ticker = tokio::spawn({
        let ticks = Arc::clone(&ticks);
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                ticks.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    let start = Instant::now();
    let result = cspcl.send_bundle_async(b"lost".to_vec(), UNREACHABLE).await;
    let elapsed = start.elapsed();
    ticker.abort();

    assert!(result.is_err(), "send to unreachable peer must fail");
    assert!(
        elapsed >= Duration::from_millis(200),
        "send failed too fast ({elapsed:?}) to tell whether it blocks the runtime"
    );
    let expected_min = (elapsed.as_millis() / 10 / 2) as u32;
    assert!(
        ticks.load(Ordering::Relaxed) >= expected_min,
        "runtime was blocked: {} ticks in {elapsed:?}",
        ticks.load(Ordering::Relaxed)
    );
}
