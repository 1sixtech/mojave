use clap::{Parser, ValueEnum};

mod throttler;
mod token_bucket;

#[derive(ValueEnum, Clone, Debug)]
pub enum TestType {
    Erc20,
}

#[derive(Parser, Clone)]
#[command(name = "load_generator")]
#[command(about = "A CLI tool for long running load testing", long_about = None)]
pub struct Cli {
    #[arg(
        long,
        short = 'n',
        default_value = "http://localhost:8545",
        help = "The URL of the full node to connect to"
    )]
    node: String,

    #[arg(long, short = 't', value_enum, default_value_t = TestType::Erc20, help = "The type of the test")]
    test_type: TestType,

    #[arg(
        long,
        short = 'd',
        default_value = "10",
        help = "The duration of the test in seconds"
    )]
    duration: u64,

    #[arg(
        long,
        short = 'r',
        default_value = "10",
        help = "The rate of requests per second"
    )]
    rps: f64,

    #[arg(long, short = 'b', help = "The burst size of requests")]
    burst: Option<usize>,

    #[arg(long, short = 'a', help = "The arrival rate of requests")]
    arrival: String,

    #[arg(long, short = 'm', help = "The maximum number of inflight requests")]
    max_inflight: usize,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
}
