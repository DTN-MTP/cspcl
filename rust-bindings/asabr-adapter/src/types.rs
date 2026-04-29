#![allow(non_camel_case_types)]

use std::os::raw::c_char;

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cspcl_route_mode_t {
    CSPCL_ROUTE_MODE_NONE = 0,
    CSPCL_ROUTE_MODE_UNICAST,
    CSPCL_ROUTE_MODE_MULTICAST,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cspcl_route_decision_status_t {
    CSPCL_ROUTE_DECISION_FOUND = 0,
    CSPCL_ROUTE_DECISION_NO_ROUTE,
    CSPCL_ROUTE_DECISION_PROVIDER_ERROR,
    CSPCL_ROUTE_DECISION_TIMEOUT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum cspcl_route_error_t {
    CSPCL_ROUTE_OK = 0,
    CSPCL_ROUTE_ERR_INVALID_PARAM,
    CSPCL_ROUTE_ERR_NOT_INITIALIZED,
    CSPCL_ROUTE_ERR_ALREADY_INITIALIZED,
    CSPCL_ROUTE_ERR_NO_PROVIDER,
    CSPCL_ROUTE_ERR_PROVIDER_FAILED,
    CSPCL_ROUTE_ERR_NO_MEMORY,
    CSPCL_ROUTE_ERR_INTERNAL,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct cspcl_route_next_hop_t {
    pub next_hop_node_id: u16,
    pub contact_identifier: u64,
    pub estimated_arrival_time: f64,
}

#[repr(C)]
pub struct cspcl_route_request_t {
    pub source_node_id: u16,
    pub destination_node_ids: *const u16,
    pub destination_count: usize,
    pub bundle_priority: i8,
    pub bundle_size: f64,
    pub bundle_expiration: f64,
    pub current_time: f64,
    pub excluded_node_ids: *const u16,
    pub excluded_node_count: usize,
    pub timeout_ms: u32,
}

#[repr(C)]
pub struct cspcl_route_provider_output_t {
    pub decision_status: cspcl_route_decision_status_t,
    pub mode: cspcl_route_mode_t,
    pub next_hops: *const cspcl_route_next_hop_t,
    pub next_hop_count: usize,
    pub diagnostic: *const c_char,
}
