//! CSP addresses of cspcl peers, as written in picod3tn's CLA addresses.

use core::{fmt, str::FromStr};

/// Highest node address accepted. CSP addresses are 5 bits
/// (`CSP_ID_HOST_SIZE`), and 31 is `CSP_BROADCAST_ADDR`.
pub const MAX_NODE_ADDR: u8 = 30;

/// Lowest port accepted. Ports 0 to 6 are CSP's built-in services (CMP,
/// ping, ps, memfree, reboot, buf_free, uptime).
pub const MIN_PORT: u8 = 7;

/// Highest port accepted: cspcl binds ports up to `CSPCL_CSP_PORT_MAX_BIND`,
/// and `csp_bind` refuses higher ones.
pub const MAX_PORT: u8 = 31;

/// The CSP node address and port a cspcl peer listens on.
///
/// Its text form is `<addr>:<port>`, e.g. `2:10`, so picod3tn writes it
/// `cspcl:2:10`. The port is required. uD3TN writes the same peer
/// `csp:2,10`, and defaults to port 10 when it is left out.
///
/// ```
/// use picod3tn_cspcl::CspclAddr;
///
/// let addr: CspclAddr = "2:10".parse().unwrap();
/// assert_eq!((addr.addr(), addr.port()), (2, 10));
/// assert_eq!(addr.to_string(), "2:10");
/// assert!("2".parse::<CspclAddr>().is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CspclAddr {
    addr: u8,
    port: u8,
}

impl CspclAddr {
    /// Checks that `addr` is a node address (0 to [`MAX_NODE_ADDR`]) and
    /// `port` a port cspcl can listen on ([`MIN_PORT`] to [`MAX_PORT`]).
    pub const fn new(addr: u8, port: u8) -> Result<Self, AddrError> {
        if addr > MAX_NODE_ADDR {
            return Err(AddrError::NodeAddr);
        }
        if port < MIN_PORT || port > MAX_PORT {
            return Err(AddrError::Port);
        }
        Ok(Self { addr, port })
    }

    /// The CSP node address.
    pub const fn addr(self) -> u8 {
        self.addr
    }

    /// The CSP port.
    pub const fn port(self) -> u8 {
        self.port
    }
}

impl FromStr for CspclAddr {
    type Err = AddrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (addr, port) = s.split_once(':').ok_or(AddrError::Format)?;
        Ok(Self {
            addr: parse_node_addr(addr)?,
            port: parse_port(port)?,
        })
    }
}

impl fmt::Display for CspclAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

/// Why a CSP address or port was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AddrError {
    /// Not of the form `<addr>:<port>`.
    #[error("expected <addr>:<port>, e.g. 2:10")]
    Format,
    /// Not a node address from 0 to [`MAX_NODE_ADDR`].
    #[error("invalid CSP node address: expected 0 to {MAX_NODE_ADDR}")]
    NodeAddr,
    /// Not a port from [`MIN_PORT`] to [`MAX_PORT`].
    #[error("invalid CSP port: expected {MIN_PORT} to {MAX_PORT}")]
    Port,
}

/// Parses a CSP node address, from 0 to [`MAX_NODE_ADDR`].
pub fn parse_node_addr(s: &str) -> Result<u8, AddrError> {
    parse_u8(s)
        .filter(|addr| *addr <= MAX_NODE_ADDR)
        .ok_or(AddrError::NodeAddr)
}

/// Parses a CSP port, from [`MIN_PORT`] to [`MAX_PORT`].
pub fn parse_port(s: &str) -> Result<u8, AddrError> {
    parse_u8(s)
        .filter(|port| (MIN_PORT..=MAX_PORT).contains(port))
        .ok_or(AddrError::Port)
}

/// Decimal digits only: `u8::from_str` also takes a leading `+`.
fn parse_u8(s: &str) -> Option<u8> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn round_trips_through_text() {
        for (text, addr, port) in [("2:10", 2, 10), ("0:7", 0, 7), ("30:31", 30, 31)] {
            let parsed: CspclAddr = text.parse().unwrap();
            assert_eq!(parsed, CspclAddr::new(addr, port).unwrap());
            assert_eq!(parsed.to_string(), text);
        }
    }

    #[test]
    fn rejects_missing_port() {
        assert_eq!("2".parse::<CspclAddr>(), Err(AddrError::Format));
    }

    #[test]
    fn rejects_empty_port() {
        assert_eq!("2:".parse::<CspclAddr>(), Err(AddrError::Port));
    }

    #[test]
    fn rejects_empty_addr() {
        assert_eq!(":10".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
    }

    #[test]
    fn rejects_extra_part() {
        assert_eq!("2:10:3".parse::<CspclAddr>(), Err(AddrError::Port));
    }

    #[test]
    fn rejects_broadcast_addr() {
        assert_eq!("31:10".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
    }

    #[test]
    fn rejects_addr_over_u8() {
        assert_eq!("256:10".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
    }

    #[test]
    fn rejects_service_port() {
        assert_eq!("2:5".parse::<CspclAddr>(), Err(AddrError::Port));
        assert_eq!("2:6".parse::<CspclAddr>(), Err(AddrError::Port));
    }

    #[test]
    fn rejects_port_over_bind_max() {
        assert_eq!("2:32".parse::<CspclAddr>(), Err(AddrError::Port));
    }

    #[test]
    fn rejects_non_numbers() {
        assert_eq!("a:b".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
        assert_eq!("+2:10".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
        assert_eq!("2:+10".parse::<CspclAddr>(), Err(AddrError::Port));
        assert_eq!(" 2:10".parse::<CspclAddr>(), Err(AddrError::NodeAddr));
    }

    #[test]
    fn new_checks_ranges() {
        assert_eq!(CspclAddr::new(31, 10), Err(AddrError::NodeAddr));
        assert_eq!(CspclAddr::new(2, 6), Err(AddrError::Port));
        assert_eq!(CspclAddr::new(2, 32), Err(AddrError::Port));
    }
}
