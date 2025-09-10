use figment::{Figment, providers::{Serialized, Toml, Env, Format}};
use mojave_node_lib::types::NodeOptions;

use crate::cli::Options;

pub(crate) fn load_config(opts: Options) -> Result<NodeOptions, figment::Error> {
    let figment = Figment::new()
        .merge(Serialized::defaults(NodeOptions::default()))
        .merge(Toml::file("Config.toml"))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Serialized::defaults(opts)).extract()?;
    Ok(figment)
}
