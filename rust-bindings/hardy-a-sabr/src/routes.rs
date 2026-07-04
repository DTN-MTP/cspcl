use hardy_bpa::routing::RouteAction;
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
