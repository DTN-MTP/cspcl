use std::time::Duration;

use cspcl::{Cspcl, CspAddress, Error, Interface};

#[test]
fn ping_unreachable_node_times_out() {
    let cspcl = Cspcl::new(
        CspAddress { addr: 1, port: 10 },
        Interface::Loopback,
    )
    .expect("init loopback cspcl");

    // Address 42 has no route on the loopback interface -> no reply.
    let result = cspcl.ping(42, Duration::from_millis(200));
    assert!(
        matches!(result, Err(Error::Timeout)),
        "expected Err(Timeout), got {result:?}"
    );
}
