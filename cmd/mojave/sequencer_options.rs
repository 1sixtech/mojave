use std::fmt;

use clap::{ArgGroup, Parser};

#[derive(Parser)]
#[clap(group(ArgGroup::new("mojave::SequencerOptions")))]
pub struct SequencerOpts {
    #[arg(
        long = "full_node.addresses",
        help = "Allowed domain(s) and port(s) for the sequencer in the form 'domain:port', can be specified multiple times",
        help_heading = "Full Node Options",
        default_value = "0.0.0.0:8545",
        value_delimiter = ','
    )]
    pub full_node_addresses: Vec<String>,
    #[arg(
        long = "block_time",
        help = "Block creation interval in milliseconds",
        default_value = "1000"
    )]
    pub block_time: u64,
}

impl Default for SequencerOpts {
    fn default() -> Self {
        Self {
            full_node_addresses: vec!["0.0.0.0:8545".to_string()],
            block_time: 1000,
        }
    }
}

impl fmt::Debug for SequencerOpts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SequencerOptions")
            .field("full_node_addresses", &self.full_node_addresses)
            .field("block_time", &self.block_time)
            .finish()
    }
}
