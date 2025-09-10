use figment::{Figment, providers::{Serialized, Json, Env, Format}};
use mojave_node_lib::types::NodeOptions;

use crate::cli::Options;

pub(crate) fn load_config(opts: Options) -> Result<NodeOptions, figment::Error> {
    let figment = Figment::new()
        .merge(Serialized::defaults(NodeOptions::default()))
        .merge(Env::prefixed("ETHREX_"))
        .merge(Json::file("mojave/node.setting.json"))
        .merge(Serialized::defaults(opts)).extract()?;
    Ok(figment)
}
