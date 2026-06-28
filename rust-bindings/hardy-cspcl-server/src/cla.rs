use hardy_cspcl::Cla;

use crate::error::ServerError;

pub fn create_cla(config: hardy_cspcl::Config) -> Result<Cla, ServerError> {
    let cla = hardy_cspcl::Cla::new(&config).map_err(ServerError::CreateCla)?;
    Ok(cla)
}
