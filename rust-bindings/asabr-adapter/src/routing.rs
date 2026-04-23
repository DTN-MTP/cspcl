use std::ffi::CString;

use a_sabr::bundle::Bundle;

use crate::state::AdapterState;
use crate::types::{
    cspcl_route_decision_status_t, cspcl_route_error_t, cspcl_route_mode_t, cspcl_route_next_hop_t,
    cspcl_route_request_t,
};

pub struct RouteDecision {
    pub decision_status: cspcl_route_decision_status_t,
    pub mode: cspcl_route_mode_t,
    pub next_hops: Vec<cspcl_route_next_hop_t>,
    pub diagnostic: CString,
}

pub(crate) fn contact_identifier(tx_node: u16, rx_node: u16, start: f64, end: f64) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in tx_node.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in rx_node.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in start.to_bits().to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for byte in end.to_bits().to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn decide_route(
    state: &mut AdapterState,
    request: &cspcl_route_request_t,
) -> Result<RouteDecision, cspcl_route_error_t> {
    if request.destination_count == 0 || request.destination_node_ids.is_null() {
        return Err(cspcl_route_error_t::CSPCL_ROUTE_ERR_INVALID_PARAM);
    }

    let destinations = unsafe {
        std::slice::from_raw_parts(request.destination_node_ids, request.destination_count)
    };
    let excluded = if request.excluded_node_count == 0 || request.excluded_node_ids.is_null() {
        Vec::new()
    } else {
        unsafe {
            std::slice::from_raw_parts(request.excluded_node_ids, request.excluded_node_count)
        }
        .to_vec()
    };

    let bundle = Bundle {
        source: request.source_node_id,
        destinations: destinations.to_vec(),
        priority: request.bundle_priority,
        size: request.bundle_size,
        expiration: request.bundle_expiration,
    };

    let routing_result = state.router.route(
        request.source_node_id,
        &bundle,
        request.current_time,
        &excluded,
    );

    let maybe_output = match routing_result {
        Ok(Some(output)) => output,
        Ok(None) => {
            return Ok(RouteDecision {
                decision_status: cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_NO_ROUTE,
                mode: cspcl_route_mode_t::CSPCL_ROUTE_MODE_NONE,
                next_hops: Vec::new(),
                diagnostic: CString::new("no-route").unwrap(),
            });
        }
        Err(_) => {
            return Ok(RouteDecision {
                decision_status: cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_PROVIDER_ERROR,
                mode: cspcl_route_mode_t::CSPCL_ROUTE_MODE_NONE,
                next_hops: Vec::new(),
                diagnostic: CString::new("provider-error").unwrap(),
            });
        }
    };

    let mut next_hops = Vec::new();

    for (_, (contact_rc, routes)) in maybe_output.first_hops.iter() {
        let contact = contact_rc.borrow();
        let first_route = routes.first().expect("route list cannot be empty");
        let route_stage = first_route.borrow();
        let next_hop = cspcl_route_next_hop_t {
            next_hop_node_id: contact.get_rx_node(),
            contact_identifier: contact_identifier(
                contact.get_tx_node(),
                contact.get_rx_node(),
                contact.info.start,
                contact.info.end,
            ),
            estimated_arrival_time: route_stage.at_time,
        };
        next_hops.push(next_hop);
    }

    if next_hops.is_empty() {
        return Ok(RouteDecision {
            decision_status: cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_NO_ROUTE,
            mode: cspcl_route_mode_t::CSPCL_ROUTE_MODE_NONE,
            next_hops,
            diagnostic: CString::new("no-route").unwrap(),
        });
    }

    let decision_mode = if next_hops.len() > 1 || request.destination_count > 1 {
        cspcl_route_mode_t::CSPCL_ROUTE_MODE_MULTICAST
    } else {
        cspcl_route_mode_t::CSPCL_ROUTE_MODE_UNICAST
    };

    Ok(RouteDecision {
        decision_status: cspcl_route_decision_status_t::CSPCL_ROUTE_DECISION_FOUND,
        mode: decision_mode,
        next_hops,
        diagnostic: CString::new("asabr-route-found").unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::contact_identifier;

    #[test]
    fn contact_identifier_is_stable() {
        let a = contact_identifier(1, 2, 10.0, 20.0);
        let b = contact_identifier(1, 2, 10.0, 20.0);
        assert_eq!(a, b);
    }
}
