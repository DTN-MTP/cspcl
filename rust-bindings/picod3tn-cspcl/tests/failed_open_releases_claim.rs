//! A link that cspcl could not start does not keep the next one from
//! opening.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use picod3tn_cspcl::{Config, CspclLink, Interface, OpenError};

#[tokio::test]
async fn failed_open_releases_the_claim() {
    let missing_can = Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Can(Some("nosuchcan9".into())),
    };
    assert!(matches!(
        CspclLink::new(&missing_can),
        Err(OpenError::Init(_))
    ));

    CspclLink::new(&Config {
        interface: Interface::Loopback,
        ..missing_can
    })
    .expect("link opens after a failed one");
}
