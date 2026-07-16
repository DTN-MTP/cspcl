use clap::{Args, Parser, ValueEnum};
use hardy_bpv7::eid::NodeId;
use std::str::FromStr;

#[derive(ValueEnum, Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum Interface {
    #[default]
    Loopback,
    Can,
}

#[derive(Args, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "kebab-case"))]
pub struct PeerConfig {
    pub node_id: NodeId,
    pub addr: u8,
    pub port: u8,
    pub heartbeat_interval: Option<u32>,
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            node_id: "ipn:1.0".parse().expect("valid default node id"),
            addr: 0,
            port: 0,
            heartbeat_interval: None,
        }
    }
}

impl FromStr for PeerConfig {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split(',');

        let node_id = parts
            .next()
            .ok_or_else(|| "missing node id".to_string())?
            .parse()
            .map_err(|err| format!("invalid node id: {err}"))?;
        let addr = parts
            .next()
            .ok_or_else(|| "missing CSP address".to_string())?
            .parse()
            .map_err(|err| format!("invalid CSP address: {err}"))?;
        let port = parts
            .next()
            .ok_or_else(|| "missing CSP port".to_string())?
            .parse()
            .map_err(|err| format!("invalid CSP port: {err}"))?;

        let heartbeat_interval = match parts.next() {
            Some(s) => Some(
                s.parse()
                    .map_err(|err| format!("invalid heartbeat interval: {err}"))?,
            ),
            None => None,
        };

        if parts.next().is_some() {
            return Err("expected NODE_ID,ADDR,PORT[,HEARTBEAT_INTERVAL]".to_string());
        }

        Ok(Self {
            node_id,
            addr,
            port,
            heartbeat_interval,
        })
    }
}

#[cfg(feature = "serde")]
fn default_failure_threshold() -> u32 {
    3
}
#[cfg(feature = "serde")]
fn default_ping_timeout_ms() -> u32 {
    1000
}
#[cfg(feature = "serde")]
fn default_heartbeat_interval_s() -> u32 {
    5
}

#[derive(Parser, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "kebab-case"))]
pub struct Config {
    #[arg(long, default_value = "1")]
    pub local_addr: u8,
    #[arg(long, default_value = "1")]
    pub port: u8,
    #[arg(long, default_value = "can")]
    pub interface: Interface,
    #[arg(long, default_value = "vcan0")]
    pub interface_name: String,
    #[arg(long = "peer", value_name = "NODE_ID,ADDR,PORT[,HEARTBEAT_INTERVAL]")]
    pub peers: Vec<PeerConfig>,
    #[arg(long, default_value = "3")]
    #[cfg_attr(feature = "serde", serde(default = "default_failure_threshold"))]
    pub failure_threshold: u32,
    #[arg(long, default_value = "1000")]
    #[cfg_attr(feature = "serde", serde(default = "default_ping_timeout_ms"))]
    pub ping_timeout_ms: u32,
    #[arg(long, default_value = "5")]
    #[cfg_attr(feature = "serde", serde(default = "default_heartbeat_interval_s"))]
    pub default_heartbeat_interval_s: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            local_addr: 1,
            port: 1,
            interface: Interface::default(),
            interface_name: "vcan0".to_string(),
            peers: Vec::new(),
            failure_threshold: 3,
            ping_timeout_ms: 1000,
            default_heartbeat_interval_s: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repeated_peer_arguments() {
        let config = Config::parse_from([
            "hardy-cspcl",
            "--local-addr",
            "1",
            "--port",
            "0",
            "--interface",
            "loopback",
            "--interface-name",
            "loopback",
            "--peer",
            "ipn:2.0,2,0",
            "--peer",
            "ipn:3.0,3,1,60",
        ]);

        assert_eq!(config.peers.len(), 2);
        assert_eq!(config.peers[0].node_id.to_string(), "ipn:2.0");
        assert_eq!(config.peers[0].addr, 2);
        assert_eq!(config.peers[0].port, 0);
        assert_eq!(config.peers[1].node_id.to_string(), "ipn:3.0");
        assert_eq!(config.peers[1].addr, 3);
        assert_eq!(config.peers[1].port, 1);
        assert_eq!(config.peers[0].heartbeat_interval, None);
        assert_eq!(config.peers[1].heartbeat_interval, Some(60));
    }

    #[test]
    fn parses_heartbeat_interval() {
        let config = Config::parse_from([
            "hardy-cspcl",
            "--interface", "loopback",
            "--interface-name", "loopback",
            "--peer", "ipn:2.0,2,0",
            "--peer", "ipn:3.0,3,1,60",
        ]);

        assert_eq!(config.peers[0].heartbeat_interval, None);
        assert_eq!(config.peers[1].heartbeat_interval, Some(60));
    }

    #[test]
    fn rejects_malformed_heartbeat_interval() {
        let err = "ipn:2.0,2,0,abc".parse::<PeerConfig>().unwrap_err();
        assert!(err.contains("heartbeat interval"), "unexpected error: {err}");
    }

    #[test]
    fn config_has_liveness_defaults() {
        let config = Config::default();
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.ping_timeout_ms, 1000);
        assert_eq!(config.default_heartbeat_interval_s, 5);
    }
}
