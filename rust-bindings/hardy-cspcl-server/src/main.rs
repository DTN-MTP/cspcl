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
    let cla = Arc::new(create_cla(config.cspcl_config)?);
    register::register_cla_to_remote_bpa(cla.clone(), config.bpa_addr).await?;
    cla.start_dispatcher()?.await;
    Ok(())
}
