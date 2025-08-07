use clap::{Parser, Subcommand};
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
    },
}
