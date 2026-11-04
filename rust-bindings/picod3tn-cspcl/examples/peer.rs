//! A cspcl peer for manual tests: prints the bundles it receives and, with
//! `--to`, sends one every second.
//!
//! ```text
//! cargo run -p picod3tn-cspcl --features cli --example peer -- \
//!     --cspcl-local-addr 1 --cspcl-interface can:vcan0 --to 2:10
//! ```

use std::time::Duration;

use clap::Parser;
use futures_util::StreamExt;
use picod3tn_cspcl::{CspclAddr, CspclArgs, CspclLink};

#[derive(Parser)]
struct Args {
    #[command(flatten)]
    cspcl: CspclArgs,

    /// Peer to send a bundle to every second.
    #[arg(long)]
    to: Option<CspclAddr>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let config = args.cspcl.config().expect("--cspcl-local-addr is required");
    let link = CspclLink::new(&config).expect("open link");
    println!(
        "listening as {}:{} on {}",
        config.local_addr, config.port, config.interface
    );

    if let Some(to) = args.to {
        let link = link.clone();
        let from = config.local_addr;
        tokio::spawn(async move {
            for n in 0u64..=u64::MAX {
                let bundle = format!("bundle {n} from {from}");
                match link.send(bundle.clone(), to).await {
                    Ok(()) => println!("sent    {bundle:?} to {to}"),
                    Err(e) => println!("failed  {bundle:?} to {to}: {e} ({:?})", e.kind()),
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    let mut incoming = link.incoming();
    while let Some(received) = incoming.next().await {
        println!(
            "received {:?} from {}:{}",
            String::from_utf8_lossy(&received.data),
            received.src_addr,
            received.src_port
        );
    }
}
