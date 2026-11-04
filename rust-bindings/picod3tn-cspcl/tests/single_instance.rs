//! Only one link opens per process; a failed open leaves the claim free.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use picod3tn_cspcl::{AddrError, Config, CspclLink, Interface, OpenError};

#[tokio::test]
async fn second_link_is_refused() {
    let loopback = Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Loopback,
    };

    // Rejected before cspcl is touched.
    let bad_port = Config {
        port: 5,
        ..loopback.clone()
    };
    assert!(matches!(
        CspclLink::new(&bad_port),
        Err(OpenError::Config(AddrError::Port))
    ));

    let _link = CspclLink::new(&loopback).expect("first link opens");
    assert!(matches!(
        CspclLink::new(&loopback),
        Err(OpenError::AlreadyOpen)
    ));
}
