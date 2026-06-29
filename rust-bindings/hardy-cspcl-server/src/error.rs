use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Could not parse the config provided: {0}")]
    ParseConfig(#[from] clap::Error),
    #[error("Could not build cla: {0}")]
    CreateCla(#[from] hardy_cspcl::Error),
    #[error("Could not register cla: {0}")]
    RegisterCla(#[from] hardy_bpa::cla::Error),
}
