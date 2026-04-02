use clap::Parser;
use std::path::PathBuf;

/// faultsim — discrete-event simulator for failure-detection research
#[derive(Parser)]
#[command(name = "faultsim", version, about)]
struct Cli {
    /// Path to scenario configuration file (TOML)
    #[arg(short, long)]
    config: PathBuf,

    /// Override the RNG seed from the config
    #[arg(short, long)]
    seed: Option<u64>,
}

fn main() {
    let cli = Cli::parse();

    // TODO: Load config from cli.config
    // TODO: Override seed if cli.seed is Some
    // TODO: Build scenario from config
    // TODO: Run simulation
    // TODO: Print or export metrics summary

    println!(
        "faultsim: would run scenario from {:?}",
        cli.config.display()
    );
}
