use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Could not parse the config provided: {0}")]
    ParseConfig(#[from] clap::Error),
    #[error("Could not read config file: {0}")]
    ReadConfig(#[from] std::io::Error),
    #[error("Could not parse config file: {0}")]
    ParseConfigFile(#[from] serde_saphyr::Error),
    #[error("Could not build cla: {0}")]
    CreateCla(#[from] hardy_cspcl::Error),
    #[error("Could not register cla: {0}")]
    RegisterCla(#[from] hardy_bpa::cla::Error),
}
