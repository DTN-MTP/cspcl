//! Safe Rust bindings for CSPCL (CubeSat Space Protocol Convergence Layer)
//!
//! This crate provides minimal safe wrappers around the C CSPCL library,
//! enabling transmission of BP7 bundles over CSP while keeping connection
//! handling explicit for higher-level integrations.

pub use cspcl_sys;

// Module declarations
mod addressing;
#[cfg(feature = "async-tokio")]
pub mod asynchronous;
mod bundle;
mod error;
mod instance;
mod interface;
mod io;

// Public exports
pub use addressing::{RemotePeer, addr_to_endpoint, endpoint_to_addr};
pub use bundle::{ReceivedBundle, ReceivedBundleView};
pub use error::{Error, Result};
pub use instance::{ConnectionStats, Cspcl, CspclConfig};
pub use interface::{Interface, InterfaceName};
pub use io::{Receiver, Sender};
