use a_sabr::errors::ASABRError;
use hardy_bpa::routing::{self, RoutingSink};

use crate::{
    engine::ShadowEngineConfig,
    projection::{ProjectionConfig, project_routes},
    routes::{ProjectedRoute, apply_route_diff, diff_routes, reconcile_installed_routes},
    topology::TopologySnapshot,
};

#[derive(Debug)]
pub enum RefreshError {
    Asabr(ASABRError),
    Hardy(routing::Error),
}

impl From<ASABRError> for RefreshError {
    fn from(error: ASABRError) -> Self {
        Self::Asabr(error)
    }
}

impl From<routing::Error> for RefreshError {
    fn from(error: routing::Error) -> Self {
        Self::Hardy(error)
    }
}

pub async fn refresh_routes(
    sink: &dyn RoutingSink,
    installed: &mut Vec<ProjectedRoute>,
    topology: &TopologySnapshot,
    engine_config: &ShadowEngineConfig,
    projection_config: &ProjectionConfig,
    source: u16,
    now: i64,
) -> Result<(), RefreshError> {
    let desired = project_routes(topology, engine_config, projection_config, source, now)?;
    let diff = diff_routes(installed, &desired);
    let accepted = apply_route_diff(sink, &diff).await?;

    reconcile_installed_routes(installed, &diff, accepted);

    Ok(())
}
