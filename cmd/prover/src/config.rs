use figment::{
    Figment,
    providers::{Env, Format, Json, Serialized},
};

use serde::{Deserialize, Serialize};

use crate::cli::Cli;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // General Options
    pub log_level: Option<String>,
    pub datadir: String,
    // Subcommands Options
    pub prover_port: u16,
    pub prover_host: String,
    pub queue_capacity: usize,
    pub aligned_mode: bool,
    pub private_key: String,
    pub no_daemon: bool,
}

// TODO: set proper defaults for work without config
impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: None,
            datadir: "./mojave/prover".to_owned(),
            prover_port: 3900,
            prover_host: "0.0.0.0".to_owned(),
            queue_capacity: 100,
            aligned_mode: false,
            no_daemon: false,
            private_key: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        }
    }
}

pub(crate) fn load_config(cli: Cli) -> Result<Config, Box<figment::Error>> {
    let figment = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Json::file("mojave/prover.setting.json"))
        .merge(Serialized::<Cli>::defaults(cli))
        .extract()?;
    Ok(figment)
}
