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
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            node_id: "ipn:1.0".parse().expect("valid default node id"),
            addr: 0,
            port: 0,
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

        if parts.next().is_some() {
            return Err("expected NODE_ID,ADDR,PORT".to_string());
        }

        Ok(Self {
            node_id,
            addr,
            port,
        })
    }
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
    #[arg(long = "peer", value_name = "NODE_ID,ADDR,PORT")]
    pub peers: Vec<PeerConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            local_addr: 1,
            port: 1,
            interface: Interface::default(),
            interface_name: "vcan0".to_string(),
            peers: Vec::new(),
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
            "ipn:3.0,3,1",
        ]);

        assert_eq!(config.peers.len(), 2);
        assert_eq!(config.peers[0].node_id.to_string(), "ipn:2.0");
        assert_eq!(config.peers[0].addr, 2);
        assert_eq!(config.peers[0].port, 0);
        assert_eq!(config.peers[1].node_id.to_string(), "ipn:3.0");
        assert_eq!(config.peers[1].addr, 3);
        assert_eq!(config.peers[1].port, 1);
    }
}
