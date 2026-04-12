use crate::address::{addr_to_endpoint, endpoint_to_addr};

/// Transport-native identity for a reachable remote CSP peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RemotePeer {
    pub addr: u8,
    pub port: u8,
}

impl RemotePeer {
    pub fn new(addr: u8, port: u8) -> Self {
        Self { addr, port }
    }

    pub fn from_endpoint(endpoint: &str, port: u8) -> Option<Self> {
        endpoint_to_addr(endpoint).map(|addr| Self { addr, port })
    }

    pub fn endpoint(&self) -> crate::Result<String> {
        addr_to_endpoint(self.addr)
    }
}
