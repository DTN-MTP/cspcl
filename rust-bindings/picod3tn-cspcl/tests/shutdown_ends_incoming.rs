//! `shutdown()` ends `incoming()` for good.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use std::time::Duration;

use futures_util::StreamExt;
use picod3tn_cspcl::{Config, CspclAddr, CspclLink, Interface};

#[tokio::test]
async fn shutdown_ends_incoming_without_reopening() {
    let link = CspclLink::new(&Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Loopback,
    })
    .expect("open link");
    let mut incoming = link.incoming();
    let me: CspclAddr = "1:10".parse().unwrap();

    // Receiving starts when the stream is first polled.
    let (sent, received) = tokio::join!(
        link.send(b"before".to_vec(), me),
        tokio::time::timeout(Duration::from_secs(5), incoming.next()),
    );
    sent.expect("loopback send");
    let received = received.expect("bundle within 5 s").expect("stream open");
    assert_eq!(received.data, b"before");

    link.shutdown();

    // Ends well before the first reopen would have happened (500 ms).
    let end = tokio::time::timeout(Duration::from_millis(400), incoming.next())
        .await
        .expect("stream ends at once");
    assert_eq!(end, None);
    assert_eq!(incoming.next().await, None);

    // A stream asked for afterwards ends too.
    let late = tokio::time::timeout(Duration::from_secs(1), link.incoming().next())
        .await
        .expect("late stream ends at once");
    assert_eq!(late, None);
}
