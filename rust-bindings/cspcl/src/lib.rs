//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides high-level safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP UDP.

// Module declarations
mod bundle;
mod error;
mod inbound;
mod instance;

// Public exports
pub use cspcl_sys::types::InterfaceConfig as Interface;
pub use error::{Error, Result};
pub use inbound::InboundStream;
pub use instance::Cspcl;

pub struct Bundle {
    pub data: Vec<u8>,
    pub src_addr: u8,
    pub src_port: u8,
}
