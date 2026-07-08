//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides high-level safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP.

// Module declarations
mod address;
mod bundle;
mod error;
mod inbound;
mod instance;
mod listener;

// Public exports
pub use address::CspAddress;
pub use cspcl_sys::types::InterfaceConfig as Interface;
pub use error::{Error, Result};
pub use inbound::InboundStream;
pub use instance::Cspcl;

/// Bundle received from an inbound CSP connection.
pub struct Bundle {
    /// Serialized bundle payload.
    pub data: Vec<u8>,
    /// Source CSP node address.
    pub src_addr: u8,
    /// Source CSP port.
    pub src_port: u8,
}
