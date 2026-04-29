use crate::routing::{RouteDecision, RouteRequest, decide_route};
use crate::state::{init_state, set_contact_plan_path as set_cp_path, with_state_mut};
use crate::types::cspcl_route_error_t;

pub fn query_route(request: &RouteRequest) -> Result<RouteDecision, cspcl_route_error_t> {
    init_state()?;

    with_state_mut(|state| decide_route(state, request))
}

pub fn set_contact_plan_path(path: Option<String>) -> Result<(), cspcl_route_error_t> {
    set_cp_path(path)
}
