use a_sabr::errors::ASABRError;
use hardy_bpa::routing::RouteAction;
use hardy_eid_patterns::EidPattern;

use crate::{
    engine::{ShadowEngineConfig, compute_first_hop},
    routes::ProjectedRoute,
    topology::TopologySnapshot,
};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepresentativeBundle {
    pub size: i64,
    pub priority: i8,
    pub expiration_horizon: i64,
}

impl Default for RepresentativeBundle {
    fn default() -> Self {
        Self {
            size: 1,
            priority: 0,
            expiration_horizon: 3_600_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DestinationProjection {
    pub pattern: EidPattern,
    pub asabr_destination: u16,
    pub route_priority: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectionConfig {
    pub bundle: RepresentativeBundle,
    pub destinations: Vec<DestinationProjection>,
}

pub fn project_routes(
    topology: &TopologySnapshot,
    engine_config: &ShadowEngineConfig,
    config: &ProjectionConfig,
    source: u16,
    now: i64,
) -> Result<Vec<ProjectedRoute>, ASABRError> {
    let mut routes = Vec::new();

    for destination in &config.destinations {
        let Some(next_hop) = compute_first_hop(
            topology,
            engine_config,
            source,
            destination.asabr_destination,
            now,
            &config.bundle,
        )?
        else {
            continue;
        };

        let Some(next_hop_eid) = topology.hardy_eid_for(next_hop) else {
            continue;
        };

        routes.push(ProjectedRoute {
            pattern: destination.pattern.clone(),
            action: RouteAction::Via(next_hop_eid),
            priority: destination.route_priority,
        });
    }

    Ok(routes)
}
