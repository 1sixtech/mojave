use clap::Parser;

#[derive(Parser)]
#[command(name = "load_generator")]
#[command(about = "A CLI tool for long running load testing", long_about = None)]
struct Cli {}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
}
