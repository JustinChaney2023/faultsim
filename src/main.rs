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

    /// Output directory for CSV results (overrides config)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let config = match faultsim::scenario::load_config(&cli.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config {:?}: {}", cli.config.display(), e);
            std::process::exit(1);
        }
    };

    let mut engine = faultsim::scenario::build_engine(&config, cli.seed);
    engine.run();

    faultsim::scenario::print_summary(&engine.metrics, config.simulation.max_ticks);

    // Determine output directory: CLI flag > config > skip.
    let output_dir = cli.output.or_else(|| {
        config
            .output
            .as_ref()
            .and_then(|o| o.dir.as_ref())
            .map(PathBuf::from)
    });

    if let Some(dir) = output_dir {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("Error creating output directory: {}", e);
            std::process::exit(1);
        }

        let scenario_name = cli
            .config
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        if let Err(e) = engine
            .metrics
            .export_detections_csv(&dir.join("detections.csv"))
        {
            eprintln!("Error writing detections CSV: {}", e);
        }

        if let Err(e) = engine.metrics.export_summary_csv(
            &dir.join("summary.csv"),
            config.simulation.max_ticks,
            scenario_name,
        ) {
            eprintln!("Error writing summary CSV: {}", e);
        }

        println!("Results exported to {}", dir.display());
    }
}
