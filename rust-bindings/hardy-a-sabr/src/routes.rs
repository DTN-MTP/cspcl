use hardy_bpa::routing::{self, RouteAction, RoutingSink};
use hardy_eid_patterns::EidPattern;

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedRoute {
    pub pattern: EidPattern,
    pub action: RouteAction,
    pub priority: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RouteDiff {
    pub remove: Vec<ProjectedRoute>,
    pub add: Vec<ProjectedRoute>,
}

pub fn diff_routes(installed: &[ProjectedRoute], desired: &[ProjectedRoute]) -> RouteDiff {
    let remove = installed
        .iter()
        .filter(|route| !desired.iter().any(|desired| desired == *route))
        .cloned()
        .collect();

    let add = desired
        .iter()
        .filter(|route| !installed.iter().any(|installed| installed == *route))
        .cloned()
        .collect();

    RouteDiff { remove, add }
}

pub async fn apply_route_diff(
    sink: &dyn RoutingSink,
    diff: &RouteDiff,
) -> routing::Result<Vec<ProjectedRoute>> {
    let mut accepted = Vec::new();

    for route in &diff.remove {
        sink.remove_route(&route.pattern, &route.action, route.priority)
            .await?;
    }

    for route in &diff.add {
        sink.add_route(route.pattern.clone(), route.action.clone(), route.priority)
            .await?;

        accepted.push(route.clone());
    }

    Ok(accepted)
}

pub fn reconcile_installed_routes(
    installed: &mut Vec<ProjectedRoute>,
    diff: &RouteDiff,
    accepted: Vec<ProjectedRoute>,
) {
    installed.retain(|route| !diff.remove.iter().any(|removed| removed == route));
    installed.extend(accepted);
}
