//! A bundle cspcl can never send is a permanent failure, and is not sent.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use std::time::Duration;

use futures_util::StreamExt;
use picod3tn_cspcl::{Config, CspclAddr, CspclLink, Interface, MAX_BUNDLE_SIZE, SendError};

#[tokio::test]
async fn oversized_bundle_is_permanent() {
    let link = CspclLink::new(&Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Loopback,
    })
    .expect("open link");
    let mut incoming = link.incoming();
    let me: CspclAddr = "1:10".parse().unwrap();

    let oversized = vec![0xaa; MAX_BUNDLE_SIZE + 1];
    assert_eq!(
        link.send(oversized, me).await,
        Err(SendError::Permanent(cspcl::Error::BundleTooLarge))
    );
    assert_eq!(
        link.send(Vec::new(), me).await,
        Err(SendError::Permanent(cspcl::Error::InvalidParam))
    );

    // Nothing was sent: the next bundle received is the next one sent.
    // Receiving starts when the stream is first polled.
    let (sent, received) = tokio::join!(
        link.send(b"after".to_vec(), me),
        tokio::time::timeout(Duration::from_secs(5), incoming.next()),
    );
    sent.expect("loopback send");
    let received = received.expect("bundle within 5 s").expect("stream open");
    assert_eq!(received.data, b"after");
}
