use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("data store disconnected")]
    ParseConfig(#[from] clap::Error),
}
