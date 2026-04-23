use std::ffi::c_void;
use std::mem;
use std::ptr::NonNull;

use crate::query::query_route;
use crate::routing::{RouteDecision, RouteRequest};
use crate::state::{reset_state, with_state_mut};
use crate::types::{cspcl_route_error_t, cspcl_route_provider_output_t, cspcl_route_request_t};

fn validate_slice_len<T>(count: usize) -> bool {
    count <= (isize::MAX as usize) / mem::size_of::<T>()
}

fn copy_nodes(ptr: *const u16, count: usize) -> Result<Vec<u16>, cspcl_route_error_t> {
    if count == 0 {
        return Ok(Vec::new());
    }

    if ptr.is_null() || !validate_slice_len::<u16>(count) {
        return Err(cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM);
    }

    // SAFETY: pointer is non-null, length was validated, and caller provides FFI buffer validity.
    Ok(unsafe { std::slice::from_raw_parts(ptr, count) }.to_vec())
}

fn request_from_ffi(request: &cspcl_route_request_t) -> Result<RouteRequest, cspcl_route_error_t> {
    let destinations = copy_nodes(request.destination_node_ids, request.destination_count)?;
    if destinations.is_empty() {
        return Err(cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM);
    }

    let excluded = copy_nodes(request.excluded_node_ids, request.excluded_node_count)?;

    Ok(RouteRequest {
        source_node_id: request.source_node_id,
        destinations,
        bundle_priority: request.bundle_priority,
        bundle_size: request.bundle_size,
        bundle_expiration: request.bundle_expiration,
        current_time: request.current_time,
        excluded,
        timeout_ms: request.timeout_ms,
    })
}

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
    let request = match NonNull::new(request as *mut cspcl_route_request_t) {
        Some(request_ptr) => {
            // SAFETY: NonNull guarantees a non-null pointer.
            unsafe { request_ptr.as_ref() }
        }
        None => return cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM,
    };

    let output = match NonNull::new(output) {
        Some(mut output_ptr) => {
            // SAFETY: NonNull guarantees a non-null pointer.
            unsafe { output_ptr.as_mut() }
        }
        None => return cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM,
    };

    let request = match request_from_ffi(request) {
        Ok(request) => request,
        Err(err) => return err,
    };

    match query_route(&request) {
        Ok(decision) => publish_decision(decision, output),
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn cspcl_asabr_route_provider_reset() {
    reset_state();
}
