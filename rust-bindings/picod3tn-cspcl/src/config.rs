//! Link configuration, and its command line / environment form.

use alloc::string::{String, ToString};
use core::{fmt, str::FromStr};

use crate::addr::{AddrError, MAX_NODE_ADDR, MAX_PORT, MIN_PORT};

/// The CAN interface used by a bare `can`. It is a virtual bus: on real
/// hardware, name the interface (e.g. `can:can0`).
pub const DEFAULT_CAN_IFACE: &str = "vcan0";

/// The ZMQ hub host used by a bare `zmqhub`.
pub const DEFAULT_ZMQHUB_HOST: &str = "localhost";

/// The CSP port listened on unless configured, as in uD3TN's examples.
pub const DEFAULT_PORT: u8 = 10;

/// Longest interface name or host cspcl takes (`CSP_IFACE_PARAM_MAX`, minus
/// the terminating NUL). Longer ones would be silently truncated.
pub const MAX_IFACE_PARAM_LEN: usize = 63;

/// Configuration of a [`CspclLink`](crate::CspclLink).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// This node's CSP address, 0 to [`MAX_NODE_ADDR`].
    pub local_addr: u8,
    /// The CSP port bundles are received on, [`MIN_PORT`] to [`MAX_PORT`].
    pub port: u8,
    /// The CSP interface.
    pub interface: Interface,
}

impl Config {
    /// Checks the address and port ranges, which the fields do not enforce.
    pub fn validate(&self) -> Result<(), AddrError> {
        if self.local_addr > MAX_NODE_ADDR {
            return Err(AddrError::NodeAddr);
        }
        if !(MIN_PORT..=MAX_PORT).contains(&self.port) {
            return Err(AddrError::Port);
        }
        Ok(())
    }
}

/// The CSP interface a link sends and receives on.
///
/// Its text form is `loopback`, `can`, `can:<iface>`, `zmqhub` or
/// `zmqhub:<host>`; a bare `can` uses [`DEFAULT_CAN_IFACE`] and a bare
/// `zmqhub` [`DEFAULT_ZMQHUB_HOST`]. Its [`Display`](fmt::Display) form
/// names the interface actually used.
///
/// ```
/// use picod3tn_cspcl::Interface;
///
/// assert_eq!("can:can0".parse(), Ok(Interface::Can(Some("can0".into()))));
/// assert_eq!("can".parse(), Ok(Interface::Can(None)));
/// assert_eq!(Interface::Can(None).to_string(), "can:vcan0");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interface {
    /// Loopback: only this node is reachable.
    Loopback,
    /// SocketCAN, on the named interface, or on [`DEFAULT_CAN_IFACE`] if
    /// none was named. That default is a virtual bus, so the link warns
    /// about it; it does not for a `vcan0` named explicitly.
    Can(Option<String>),
    /// ZMQ hub, at the named host.
    Zmqhub(String),
}

impl FromStr for Interface {
    type Err = InterfaceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (kind, param) = match s.split_once(':') {
            Some((kind, param)) => (kind, Some(param)),
            None => (s, None),
        };
        let checked_param = || match param {
            None => Ok(None),
            Some("") => Err(InterfaceError::EmptyParam),
            Some(param) if param.len() > MAX_IFACE_PARAM_LEN => Err(InterfaceError::ParamTooLong),
            Some(param) if param.contains('\0') => Err(InterfaceError::Nul),
            Some(param) => Ok(Some(param.to_string())),
        };
        match kind {
            "loopback" if param.is_none() => Ok(Self::Loopback),
            "loopback" => Err(InterfaceError::LoopbackParam),
            "can" => checked_param().map(Self::Can),
            "zmqhub" => checked_param()
                .map(|host| Self::Zmqhub(host.unwrap_or_else(|| DEFAULT_ZMQHUB_HOST.to_string()))),
            _ => Err(InterfaceError::Unknown),
        }
    }
}

impl fmt::Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Loopback => f.write_str("loopback"),
            Self::Can(iface) => write!(f, "can:{}", iface.as_deref().unwrap_or(DEFAULT_CAN_IFACE)),
            Self::Zmqhub(host) => write!(f, "zmqhub:{host}"),
        }
    }
}

/// Why an interface was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InterfaceError {
    /// Not `loopback`, `can` or `zmqhub`.
    #[error("unknown CSP interface: expected loopback, can[:<iface>] or zmqhub[:<host>]")]
    Unknown,
    /// `loopback` takes no parameter.
    #[error("the loopback interface takes no parameter")]
    LoopbackParam,
    /// A `:` with nothing after it.
    #[error("empty CSP interface name or host after `:`")]
    EmptyParam,
    /// The name or host is longer than [`MAX_IFACE_PARAM_LEN`].
    #[error("CSP interface name or host longer than {MAX_IFACE_PARAM_LEN} bytes")]
    ParamTooLong,
    /// The name or host contains a NUL byte.
    #[error("CSP interface name or host contains a NUL byte")]
    Nul,
}

/// The cspcl arguments of picod3tn, to `#[command(flatten)]` into its own.
///
/// The CLA is enabled by `--cspcl-local-addr`, which then needs
/// `--cspcl-interface` too.
#[cfg(feature = "cli")]
#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
pub struct CspclArgs {
    /// CSP address of this node (0 to 30). Enables the cspcl CLA.
    #[arg(
        id = "cspcl_local_addr",
        long = "cspcl-local-addr",
        env = "PICOD3TN_CSPCL_LOCAL_ADDR",
        value_parser = crate::addr::parse_node_addr,
        requires = "cspcl_interface"
    )]
    pub local_addr: Option<u8>,

    /// CSP port bundles are received on (7 to 31).
    #[arg(
        id = "cspcl_port",
        long = "cspcl-port",
        env = "PICOD3TN_CSPCL_PORT",
        default_value_t = DEFAULT_PORT,
        value_parser = crate::addr::parse_port
    )]
    pub port: u8,

    /// The CSP interface, see [`Interface`].
    #[arg(
        id = "cspcl_interface",
        help = "CSP interface: loopback, can[:IFACE] (default vcan0) or zmqhub[:HOST] (default localhost)",
        long = "cspcl-interface",
        env = "PICOD3TN_CSPCL_INTERFACE",
        requires = "cspcl_local_addr"
    )]
    pub interface: Option<Interface>,
}

#[cfg(feature = "cli")]
impl CspclArgs {
    /// The link configuration, or `None` if the CLA is not enabled.
    pub fn config(&self) -> Option<Config> {
        Some(Config {
            local_addr: self.local_addr?,
            port: self.port,
            interface: self.interface.clone()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loopback() {
        assert_eq!("loopback".parse(), Ok(Interface::Loopback));
        assert_eq!(
            "loopback:x".parse::<Interface>(),
            Err(InterfaceError::LoopbackParam)
        );
    }

    #[test]
    fn parses_can() {
        assert_eq!("can".parse(), Ok(Interface::Can(None)));
        assert_eq!("can:can0".parse(), Ok(Interface::Can(Some("can0".into()))));
        assert_eq!(
            "can:vcan0".parse(),
            Ok(Interface::Can(Some("vcan0".into())))
        );
        assert_eq!("can:".parse::<Interface>(), Err(InterfaceError::EmptyParam));
    }

    #[test]
    fn parses_zmqhub() {
        assert_eq!("zmqhub".parse(), Ok(Interface::Zmqhub("localhost".into())));
        assert_eq!(
            "zmqhub:10.0.0.2".parse(),
            Ok(Interface::Zmqhub("10.0.0.2".into()))
        );
        assert_eq!(
            "zmqhub:".parse::<Interface>(),
            Err(InterfaceError::EmptyParam)
        );
    }

    #[test]
    fn rejects_unknown_interfaces() {
        for s in ["", "CAN", "zmq", "kiss", ":can0"] {
            assert_eq!(s.parse::<Interface>(), Err(InterfaceError::Unknown), "{s}");
        }
    }

    #[test]
    fn rejects_bad_params() {
        let long = alloc::format!("can:{}", "x".repeat(MAX_IFACE_PARAM_LEN + 1));
        assert_eq!(long.parse::<Interface>(), Err(InterfaceError::ParamTooLong));
        let longest = alloc::format!("can:{}", "x".repeat(MAX_IFACE_PARAM_LEN));
        assert!(longest.parse::<Interface>().is_ok());
        assert_eq!("can:a\0b".parse::<Interface>(), Err(InterfaceError::Nul));
    }

    #[test]
    fn interface_round_trips_through_text() {
        for s in ["loopback", "can:can0", "zmqhub:localhost"] {
            assert_eq!(s.parse::<Interface>().unwrap().to_string(), s);
        }
        // Defaults show as the interface used.
        assert_eq!("can".parse::<Interface>().unwrap().to_string(), "can:vcan0");
        assert_eq!(
            "zmqhub".parse::<Interface>().unwrap().to_string(),
            "zmqhub:localhost"
        );
    }

    #[test]
    fn validates_config_ranges() {
        let config = |local_addr, port| Config {
            local_addr,
            port,
            interface: Interface::Loopback,
        };
        assert_eq!(config(1, 10).validate(), Ok(()));
        assert_eq!(config(31, 10).validate(), Err(AddrError::NodeAddr));
        assert_eq!(config(1, 6).validate(), Err(AddrError::Port));
        assert_eq!(config(1, 32).validate(), Err(AddrError::Port));
    }

    #[cfg(feature = "cli")]
    mod cli {
        use super::super::*;
        use clap::Parser;

        #[derive(Parser, Debug)]
        struct Args {
            #[command(flatten)]
            cspcl: CspclArgs,
        }

        fn parse(args: &[&str]) -> Result<CspclArgs, clap::Error> {
            Args::try_parse_from(core::iter::once("picod3tn").chain(args.iter().copied()))
                .map(|args| args.cspcl)
        }

        #[test]
        fn disabled_without_local_addr() {
            let args = parse(&[]).unwrap();
            assert_eq!(args.config(), None);
            assert_eq!(args.port, DEFAULT_PORT);
        }

        #[test]
        fn enabled_with_local_addr_and_interface() {
            let args = parse(&[
                "--cspcl-local-addr",
                "2",
                "--cspcl-port",
                "12",
                "--cspcl-interface",
                "can:can0",
            ])
            .unwrap();
            assert_eq!(
                args.config(),
                Some(Config {
                    local_addr: 2,
                    port: 12,
                    interface: Interface::Can(Some("can0".into())),
                })
            );
        }

        #[test]
        fn local_addr_and_interface_need_each_other() {
            assert!(parse(&["--cspcl-local-addr", "2"]).is_err());
            assert!(parse(&["--cspcl-interface", "loopback"]).is_err());
        }

        #[test]
        fn rejects_out_of_range_values() {
            let with = |addr, port| {
                parse(&[
                    "--cspcl-local-addr",
                    addr,
                    "--cspcl-port",
                    port,
                    "--cspcl-interface",
                    "loopback",
                ])
            };
            assert!(with("2", "10").is_ok());
            assert!(with("31", "10").is_err());
            assert!(with("2", "6").is_err());
            assert!(with("2", "32").is_err());
        }
    }
}
