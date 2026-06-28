mod cla;
mod config;
mod error;

use crate::{
    cla::create_cla,
    error::ServerError::{self},
};
use clap::Parser;

fn main() -> Result<(), ServerError> {
    let config = config::Config::parse();
    let _ = create_cla(config.cspcl_config)?;
    println!("Hello, world!");
    Ok(())
}
