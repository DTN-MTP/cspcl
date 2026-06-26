mod config;

use clap::Parser;
use hardy_bpa::bpa::BpaRegistration;
use hardy_proto::client::RemoteBpa;
use std::sync::Arc;

pub async fn register_cla_to_remote_bpa(cla: Arc<hardy_cspcl::Cla>) -> hardy_bpa::cla::Result<()> {
    let bpa = RemoteBpa::new("http://127.0.0.1:50051".to_string());

    bpa.register_cla("cspcl".to_string(), cla, None).await?;

    Ok(())
}

fn main() {
    let conf = config::Config::parse();
    dbg!(conf);
    println!("Hello, world!");
}
