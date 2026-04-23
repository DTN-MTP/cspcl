use crate::routing::{RouteDecision, decide_route};
use crate::state::{init_state, with_state_mut};
use crate::types::{cspcl_route_error_t, cspcl_route_request_t};

pub fn query_route(request: &cspcl_route_request_t) -> Result<RouteDecision, cspcl_route_error_t> {
    init_state()?;

    with_state_mut(|state| decide_route(state, request))
}
