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

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    let config = config::Config::parse();
    let server_config = config::load_server_config(&config.config_path)?;
    let cla = Arc::new(create_cla(server_config.cspcl_config)?);
    register::register_cla_to_remote_bpa(cla.clone(), server_config.bpa_addr).await?;
    let _ = cla.start_dispatcher()?.await;
    Ok(())
}
