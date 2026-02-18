//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides high-level safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP UDP.

pub use cspcl_sys;

// Module declarations
mod error;
mod instance;
mod bundle;
mod address;
mod utils;

// Public exports
pub use error::{Error, Result};
pub use instance::Cspcl;
pub use address::{endpoint_to_addr, addr_to_endpoint};
pub use utils::get_time_ms;
