mod ffi;
pub mod query;
mod routing;
mod state;
mod types;

pub use ffi::{cspcl_asabr_route_provider, cspcl_asabr_route_provider_reset};
pub use query::{query_route, set_contact_plan_path};
pub use routing::{RouteDecision, RouteRequest};
pub use types::{
    cspcl_route_decision_status_t, cspcl_route_error_t, cspcl_route_mode_t, cspcl_route_next_hop_t,
    cspcl_route_provider_output_t, cspcl_route_request_t,
};
