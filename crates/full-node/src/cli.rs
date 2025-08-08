use clap::{Parser, Subcommand};
use mojave_chain_utils::options::Options;
use tracing::Level;

#[allow(clippy::upper_case_acronyms)]
#[derive(Parser)]
#[command(
    name = "mojave-full-node",
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
        full_node_options: FullNodeOptions,
    },
}

#[derive(Parser)]
pub struct FullNodeOptions {
    #[arg(
        long = "sequencer.address",
        default_value = "0.0.0.0:1739",
        help = "Allowed domain and port for the sequencer in the form 'domain:port'",
        help_heading = "Full Node Options",
        required = true
    )]
    pub sequencer_address: String,
}

impl Default for FullNodeOptions {
    fn default() -> Self {
        Self {
            sequencer_address: "0.0.0.0:1739".to_string(),
        }
    }
}

impl std::fmt::Debug for FullNodeOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FullNodeOptions")
            .field("sequencer_address", &self.sequencer_address)
            .finish()
    }
}
