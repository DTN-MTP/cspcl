mod cla;
mod config;
mod error;
mod register;

use crate::error::ServerError;
use clap::Parser;

fn main() -> Result<(), ServerError> {
    let conf = config::Config::parse();

    dbg!(conf);
    println!("Hello, world!");
    Ok(())
}
