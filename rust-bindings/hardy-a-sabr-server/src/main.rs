use std::{path::PathBuf, sync::Arc};

use clap::Parser;
use hardy_a_sabr::config::RuntimeConfig;
use hardy_bpa::bpa::BpaRegistration;
use hardy_proto::client::RemoteBpa;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "hardy-a-sabr-server")]
#[command(about = "Run a remote Hardy A-SABR routing agent over gRPC")]
struct Cli {
    #[arg(short, long, value_name = "PATH")]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Config {
    grpc_addr: String,
    #[serde(default = "default_agent_name")]
    agent_name: String,
    runtime: RuntimeConfig,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info,hardy_a_sabr=debug,hardy_a_sabr_server=debug".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = load_config(&cli.config)?;

    let agent_name = config.agent_name;
    let grpc_addr = config.grpc_addr;
    let router = Arc::new(config.runtime.into_router());
    let remote_bpa = RemoteBpa::new(grpc_addr.clone());

    let node_ids = remote_bpa
        .register_routing_agent(agent_name.clone(), router.clone())
        .await?;

    info!(
        agent = %agent_name,
        grpc_addr = %grpc_addr,
        node_ids = ?node_ids,
        "A-SABR routing agent registered with remote BPA"
    );

    tokio::signal::ctrl_c().await?;

    info!(agent = %agent_name, "A-SABR routing agent shutting down");

    Ok(())
}

fn load_config(path: &PathBuf) -> Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&contents)?)
}

fn default_agent_name() -> String {
    "a-sabr".into()
}
