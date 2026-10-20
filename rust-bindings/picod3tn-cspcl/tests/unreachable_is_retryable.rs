//! A send to a peer that is not there is a retryable failure, so the bundle
//! is deferred rather than dropped.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use picod3tn_cspcl::{Config, CspclLink, FailureKind, Interface};

#[tokio::test]
async fn unreachable_peer_is_retryable() {
    let link = CspclLink::new(&Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Loopback,
    })
    .expect("open link");

    // No such node on the loopback interface.
    let error = link
        .send(b"lost".to_vec(), "9:10".parse().unwrap())
        .await
        .expect_err("no peer at 9");
    assert_eq!(error.kind(), FailureKind::Retryable, "{error}");
}
