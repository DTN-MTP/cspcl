use hardy_bpa::bpa::BpaRegistration;
use hardy_proto::client::RemoteBpa;
use std::sync::Arc;

use crate::error::ServerError;

pub async fn register_cla_to_remote_bpa(
    cla: Arc<hardy_cspcl::Cla>,
    bpa_addr: String,
) -> Result<(), ServerError> {
    let bpa = RemoteBpa::new(bpa_addr);

    bpa.register_cla("cspcl".to_string(), cla, None)
        .await
        .map_err(ServerError::RegisterCla)?;

    Ok(())
}
