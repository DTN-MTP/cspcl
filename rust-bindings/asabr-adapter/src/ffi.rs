use std::ffi::c_void;

use crate::query::query_route;
use crate::routing::RouteDecision;
use crate::state::{reset_state, with_state_mut};
use crate::types::{cspcl_route_error_t, cspcl_route_provider_output_t, cspcl_route_request_t};

fn publish_decision(
    decision: RouteDecision,
    output: &mut cspcl_route_provider_output_t,
) -> cspcl_route_error_t {
    with_state_mut(|state| {
        state.next_hops = decision.next_hops;
        state.diagnostic = decision.diagnostic;
        output.decision_status = decision.decision_status;
        output.mode = decision.mode;
        output.next_hops = state.next_hops.as_ptr();
        output.next_hop_count = state.next_hops.len();
        output.diagnostic = state.diagnostic.as_ptr();
    });

    cspcl_route_error_t::CSPCL_ROUTE_OK
}

#[unsafe(no_mangle)]
pub extern "C" fn cspcl_asabr_route_provider(
    request: *const cspcl_route_request_t,
    output: *mut cspcl_route_provider_output_t,
    _user_ctx: *mut c_void,
) -> cspcl_route_error_t {
    if request.is_null() || output.is_null() {
        return cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM;
    }

    match query_route(unsafe { &*request }) {
        Ok(decision) => publish_decision(decision, unsafe { &mut *output }),
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cspcl_asabr_route_provider_reset() {
    reset_state();
}
