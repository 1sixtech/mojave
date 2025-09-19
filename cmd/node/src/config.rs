use figment::{
    Figment,
    providers::{Env, Format, Json, Serialized},
};
use mojave_node_lib::types::{Node, SyncMode};
use mojave_utils::network::Network;
use serde::{Deserialize, Serialize};

use crate::cli::Cli;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // General Options
    pub log_level: Option<String>,
    pub datadir: String,
    // Subcommands Options
    pub network: Network,
    pub bootnodes: Vec<Node>,
    pub syncmode: SyncMode,
    pub sponsorable_addresses_file_path: Option<String>,
    pub force: bool,
    pub metrics_addr: String,
    pub metrics_port: String,
    pub metrics_enabled: bool,
    pub http_addr: String,
    pub http_port: String,
    pub authrpc_addr: String,
    pub authrpc_port: String,
    pub authrpc_jwtsecret: String,
    pub p2p_enabled: bool,
    pub p2p_addr: String,
    pub p2p_port: String,
    pub discovery_addr: String,
    pub discovery_port: String,
    pub no_daemon: bool,
}

// TODO: set proper defaults for work without config
impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: None,
            datadir: "./mojave/node".to_owned(),
            http_addr: "0.0.0.0".to_owned(),
            http_port: "8545".to_owned(),
            authrpc_addr: "localhost".to_owned(),
            authrpc_port: "8551".to_owned(),
            authrpc_jwtsecret: "jwt.hex".to_owned(),
            p2p_enabled: true,
            p2p_addr: "0.0.0.0".to_owned(),
            p2p_port: "30303".to_owned(),
            discovery_addr: "0.0.0.0".to_owned(),
            discovery_port: "30303".to_owned(),
            network: Network::DefaultNet,
            bootnodes: vec![],
            syncmode: SyncMode::Full,
            sponsorable_addresses_file_path: None,
            metrics_addr: "0.0.0.0".to_owned(),
            metrics_port: "9090".to_owned(),
            metrics_enabled: false,
            force: false,
            no_daemon: false,
        }
    }
}

impl From<&Config> for mojave_node_lib::types::NodeOptions {
    fn from(config: &Config) -> Self {
        Self {
            network: config.network.clone(),
            bootnodes: config.bootnodes.clone(),
            syncmode: config.syncmode,
            sponsorable_addresses_file_path: config.sponsorable_addresses_file_path.clone(),
            datadir: config.datadir.clone(),
            force: config.force,
            metrics_addr: config.metrics_addr.clone(),
            metrics_port: config.metrics_port.clone(),
            metrics_enabled: config.metrics_enabled,
            http_addr: config.http_addr.clone(),
            http_port: config.http_port.clone(),
            authrpc_addr: config.authrpc_addr.clone(),
            authrpc_port: config.authrpc_port.clone(),
            authrpc_jwtsecret: config.authrpc_jwtsecret.clone(),
            p2p_enabled: config.p2p_enabled,
            p2p_addr: config.p2p_addr.clone(),
            p2p_port: config.p2p_port.clone(),
            discovery_addr: config.discovery_addr.clone(),
            discovery_port: config.discovery_port.clone(),
        }
    }
}

pub(crate) fn load_config(cli: Cli) -> Result<Config, Box<figment::Error>> {
    let figment = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Json::file("mojave/node.setting.json"))
        .merge(Serialized::<Cli>::defaults(cli))
        .extract()?;

    println!("{:#?}", figment);

    Ok(figment)
}
