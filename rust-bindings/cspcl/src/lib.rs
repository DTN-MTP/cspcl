//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides high-level safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP UDP.

// Module declarations
mod address;
mod bundle;
mod error;
mod inbound;
mod instance;
mod interface;

// Public exports
pub use address::{addr_to_endpoint, endpoint_to_addr};
pub use error::{Error, Result};
pub use inbound::InboundStream;
pub use instance::Cspcl;
pub use interface::Interface;

pub struct Bundle {
    pub data: Vec<u8>,
    pub src_addr: u8,
    pub src_port: u8,
}
