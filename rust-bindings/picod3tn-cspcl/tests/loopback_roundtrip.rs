//! A bundle sent to this node over loopback comes back on `incoming()`.
//!
//! libcsp is a process-wide singleton: one link per test binary.

use std::time::Duration;

use futures_util::StreamExt;
use picod3tn_cspcl::{Config, CspclAddr, CspclLink, Interface};

#[tokio::test]
async fn send_to_self_is_received() {
    let link = CspclLink::new(&Config {
        local_addr: 1,
        port: 10,
        interface: Interface::Loopback,
    })
    .expect("open link");
    let mut incoming = link.incoming();
    let me: CspclAddr = "1:10".parse().unwrap();

    // One fragment, then many (SFP).
    for len in [32, 20_000] {
        let bundle: Vec<u8> = (0..len).map(|i| i as u8).collect();
        // Receiving starts when the stream is first polled.
        let (sent, received) = tokio::join!(
            link.send(bundle.clone(), me),
            tokio::time::timeout(Duration::from_secs(5), incoming.next()),
        );
        sent.expect("loopback send");
        let received = received.expect("bundle within 5 s").expect("stream open");
        assert_eq!(received.data, bundle);
        assert_eq!(received.src_addr, me.addr());
    }
}
