mod cla;
mod config;
mod error;
mod register;

use std::sync::Arc;

use crate::{
    cla::create_cla,
    error::ServerError::{self},
};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = config::Config::parse();
    let server_config = config::load_server_config(&config.config_path)?;
    let cla = Arc::new(create_cla(server_config.cspcl_config)?);
    register::register_cla_to_remote_bpa(cla.clone(), server_config.bpa_addr).await?;
    cla.start_dispatcher()?.await?;
    Ok(())
}
