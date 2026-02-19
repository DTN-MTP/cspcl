//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides high-level safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP UDP.

pub use cspcl_sys;

// Module declarations
mod address;
mod bundle;
mod error;
mod instance;

// Public exports
pub use address::{addr_to_endpoint, endpoint_to_addr};
pub use error::{Error, Result};
pub use instance::Cspcl;
