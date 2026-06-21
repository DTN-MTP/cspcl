use hardy_bpv7::eid::NodeId;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum Interface {
    Loopback,
    Can,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "kebab-case"))]
pub struct PeerConfig {
    pub node_id: NodeId,
    pub addr: u8,
    pub port: u8,
    pub heartbeat_interval: Option<u64>,
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

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, rename_all = "kebab-case"))]
pub struct Config {
    pub local_addr: u8,
    pub port: u8,
    pub interface: Interface,
    pub interface_name: String,
    pub peers: Vec<PeerConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            local_addr: 1,
            port: 0,
            interface: Interface::Loopback,
            interface_name: "loopback".to_string(),
            peers: Vec::new(),
        }
    }
}
