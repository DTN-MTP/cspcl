//! A cspcl link for picod3tn: bundles over CSP, with uD3TN's convergence
//! layer.
//!
//! This crate works on serialized bundles. It owns the CSP link: peer
//! addresses ([`CspclAddr`]), configuration ([`Config`], and `CspclArgs`
//! for the command line with the `cli` feature), sends whose failures say
//! whether to retry ([`SendError`]), and a receive stream that does not stop
//! while the link is open ([`CspclLink::incoming`]). The bundle spool lives
//! in picod3tn.
//!
//! # Features
//! - `std` (default): the standard library. Without it, the crate is
//!   `no_std` (with `alloc`) and only has the pure part: addresses,
//!   configuration and failure classification.
//! - `tokio` (default): the link, through the `cspcl` bindings, on a tokio
//!   runtime.
//! - `cli`: `CspclArgs`, `clap` arguments to flatten into picod3tn's.
//!
//! # Limits
//! - **One link per process.** libcsp is a process-wide singleton:
//!   [`CspclLink::new`] succeeds once per process. Share the link between
//!   the tasks that send and receive, and keep it for the life of the
//!   process.
//! - **Bundles up to [`MAX_BUNDLE_SIZE`] (65535) bytes.** A larger one is a
//!   [`SendError::Permanent`] failure. A peer never acknowledges a larger
//!   one either, so it fails the same way on every retry.
//! - **Sends are serialized.** One to an unreachable peer holds every other
//!   back for up to about 20 s (ack timeout plus one reconnect).
//! - **Recovery takes time.** The first send to a peer that restarted can
//!   fail or take 10 to 20 s on the stale connection before cspcl
//!   reconnects. Failures meanwhile (`Timeout`, `CspSend`, `Connection`) are
//!   [`SendError::Retryable`]: defer the bundle and try again.
//! - **The source port of a received bundle is ephemeral.** It is not the
//!   port the peer listens on: never reply to it, nor look the peer up by it.
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

/// The identifier of this convergence layer in picod3tn's CLA addresses
/// (`cspcl:<addr>:<port>`) and in its spool (`spool.transmit(CLA_ID)`).
pub const CLA_ID: &str = "cspcl";

mod addr;
mod config;
mod error;
#[cfg(feature = "tokio")]
mod link;
#[cfg(feature = "tokio")]
mod rx;

pub use addr::{
    AddrError, CspclAddr, MAX_NODE_ADDR, MAX_PORT, MIN_PORT, parse_node_addr, parse_port,
};
#[cfg(feature = "cli")]
pub use config::CspclArgs;
pub use config::{
    Config, DEFAULT_CAN_IFACE, DEFAULT_PORT, DEFAULT_ZMQHUB_HOST, Interface, InterfaceError,
    MAX_IFACE_PARAM_LEN,
};
#[cfg(feature = "tokio")]
pub use error::SendError;
pub use error::{FailureKind, MAX_BUNDLE_SIZE, check_size};
#[cfg(feature = "tokio")]
pub use link::{CspclLink, OpenError};
#[cfg(feature = "tokio")]
pub use rx::Received;
