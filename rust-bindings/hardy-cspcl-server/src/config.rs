use clap::Parser;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

use crate::error::ServerError;

#[derive(Parser, Debug)]
#[group(id = "cspcla")]
pub struct Config {
    #[arg(long, default_value = "~/.config/cspcl/config.yaml")]
    pub config_path: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    #[serde(default = "default_bpa_addr")]
    pub bpa_addr: String,
    #[serde(flatten)]
    pub cspcl_config: hardy_cspcl::Config,
}

pub fn load_server_config(path: &Path) -> Result<ServerConfig, ServerError> {
    let file = File::open(expand_tilde(path))?;
    Ok(serde_saphyr::from_reader(file)?)
}

fn default_bpa_addr() -> String {
    "http://127.0.0.1:51052".to_string()
}

fn expand_tilde(path: &Path) -> PathBuf {
    let Some(path) = path.to_str() else {
        return path.to_path_buf();
    };

    match path {
        "~" => std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path)),
        path if path.starts_with("~/") => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(&path[2..]))
            .unwrap_or_else(|| PathBuf::from(path)),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::fs;

    #[test]
    fn clap_command_definition_is_valid() {
        Config::command().debug_assert();
    }

    #[test]
    fn loads_server_config_from_yaml_file() {
        let path = std::env::temp_dir().join(format!(
            "hardy-cspcl-server-config-{}.yaml",
            std::process::id()
        ));
        fs::write(
            &path,
            "\
local-addr: 7
port: 9
bpa-addr: http://127.0.0.1:51052
interface: loopback
interface-name: loopback
peers:
  - node-id: ipn:2.0
    addr: 2
    port: 1
",
        )
        .expect("write test config");

        let config = load_server_config(&path).expect("load server config");
        let _ = fs::remove_file(&path);

        assert_eq!(config.bpa_addr, "http://127.0.0.1:51052");
        assert_eq!(config.cspcl_config.local_addr, 7);
        assert_eq!(config.cspcl_config.port, 9);
        assert_eq!(config.cspcl_config.interface_name, "loopback");
        assert_eq!(config.cspcl_config.peers.len(), 1);
        assert_eq!(config.cspcl_config.peers[0].node_id.to_string(), "ipn:2.0");
        assert_eq!(config.cspcl_config.peers[0].addr, 2);
        assert_eq!(config.cspcl_config.peers[0].port, 1);
    }

    #[test]
    fn liveness_tunables_default_when_absent_from_yaml() {
        let path = std::env::temp_dir().join(format!(
            "hardy-cspcl-server-defaults-{}.yaml",
            std::process::id()
        ));
        fs::write(
            &path,
            "\
local-addr: 7
port: 9
interface: loopback
interface-name: loopback
peers:
  - node-id: ipn:2.0
    addr: 2
    port: 1
",
        )
        .expect("write test config");

        let config = load_server_config(&path).expect("load server config");
        let _ = fs::remove_file(&path);

        // Omitted tunables must fall back to the field-level serde defaults,
        // NOT u32::default() (0), which flatten + container-default would give.
        assert_eq!(config.cspcl_config.failure_threshold, 3);
        assert_eq!(config.cspcl_config.ping_timeout_ms, 1000);
        assert_eq!(config.cspcl_config.default_heartbeat_interval_s, 5);
        assert_eq!(config.cspcl_config.peers[0].heartbeat_interval, None);
    }
}
