use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[group(id = "cspcla")]
pub struct Config {
    #[arg(long, default_value = "~/.config/cspcl/config.yaml")]
    pub config_path: std::path::PathBuf,
    #[arg(long, default_value = "http://127.0.0.1:51052")]
    pub bpa_addr: SocketAddr,
    #[command(flatten)]
    pub cspcl_config: hardy_cspcl::Config,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_command_definition_is_valid() {
        Config::command().debug_assert();
    }
}
