use clap::{ArgGroup, Parser, Subcommand};
use mojave_chain_utils::options::Options;
use tracing::Level;

#[allow(clippy::upper_case_acronyms)]
#[derive(Parser)]
#[command(
    name = "mojave-sequencer",
    author,
    version,
    about = "Mojave is a blockchain node implementation for the Mojave network",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(
      long = "log.level",
      default_value_t = Level::INFO,
      value_name = "LOG_LEVEL",
      help = "The verbosity level used for logs.",
      long_help = "Possible values: info, debug, trace, warn, error",
      help_heading = "Node options")]
    pub log_level: Level,
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run() -> Self {
        Self::parse()
    }
}

#[derive(Subcommand)]
pub enum Command {
    #[command(name = "init", about = "Run the sequencer")]
    Init {
        #[command(flatten)]
        options: Options,
        #[command(flatten)]
        sequencer_options: SequencerOpts,
    },
}

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

impl std::fmt::Debug for SequencerOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SequencerOptions")
            .field("full_node_addresses", &self.full_node_addresses)
            .field("block_time", &self.block_time)
            .finish()
    }
}
