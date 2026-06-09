use crate::cspcl_sys;
use crate::error::{Error, Result};
use std::ffi::CString;

/// Convert BP endpoint ID (e.g., "ipn:1.0") to CSP address
///
/// Supports IPN scheme: ipn:X.Y → CSP address X
///
/// # Arguments
/// * `endpoint` - BP endpoint ID string
///
/// # Returns
/// Some(address) if conversion successful, None otherwise
pub fn endpoint_to_addr(endpoint: &str) -> Option<u8> {
    let c_str = CString::new(endpoint).ok()?;
    let addr = unsafe { cspcl_sys::cspcl_endpoint_to_addr(c_str.as_ptr()) };
    if addr == 0 { None } else { Some(addr) }
}

/// Convert CSP address to BP endpoint ID
///
/// # Arguments
/// * `addr` - CSP address
///
/// # Returns
/// Ok(endpoint_string) on success, Err(Error) otherwise
pub fn addr_to_endpoint(addr: u8) -> Result<String> {
    // TODO: Consider if 32 bytes is enough for endpoint strings, or make it configurable
    let mut buffer = [0u8; 32];
    unsafe {
        Error::from_code(cspcl_sys::cspcl_addr_to_endpoint(
            addr,
            buffer.as_mut_ptr() as *mut i8,
            buffer.len(),
        ))?;
    }
    Ok(String::from_utf8_lossy(&buffer)
        .trim_matches('\0')
        .to_string())
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_conversion() {
        let endpoint = "ipn:1.0";
        if let Some(addr) = endpoint_to_addr(endpoint) {
            assert_eq!(addr, 1);
        }
    }
}
