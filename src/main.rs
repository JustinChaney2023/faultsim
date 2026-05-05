use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;

use faultsim::aggregate::{export_runs_csv, export_sweep_csv, AggregatedMetrics, RunSnapshot};
use faultsim::config::ScenarioConfig;

/// faultsim — discrete-event simulator for failure-detection research
#[derive(Parser)]
#[command(name = "faultsim", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a single scenario from a TOML config file
    Run(RunArgs),
    /// Run all scenario TOML files in a directory and write a combined summary
    RunAll(RunAllArgs),
    /// Run the same scenario with multiple seeds to produce confidence intervals
    SweepSeeds(SweepSeedsArgs),
    /// Vary one detector or network parameter over a range, one run per step
    Sweep(SweepArgs),
}

// ── Subcommand argument structs ───────────────────────────────────────────────

#[derive(Args)]
struct RunArgs {
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

#[derive(Args)]
struct RunAllArgs {
    /// Directory containing scenario TOML files
    #[arg(short, long)]
    scenarios: PathBuf,
    /// Output directory for CSV results
    #[arg(short, long)]
    output: PathBuf,
}

#[derive(Args)]
struct SweepSeedsArgs {
    /// Path to scenario configuration file (TOML)
    #[arg(short, long)]
    config: PathBuf,
    /// Number of seeds to run (seeds 1 through N)
    #[arg(long, default_value = "30")]
    seeds: u64,
    /// Output directory for per-run and aggregated CSVs
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Args)]
struct SweepArgs {
    /// Path to scenario configuration file (TOML)
    #[arg(short, long)]
    config: PathBuf,
    /// Parameter to sweep (e.g. phi_threshold, timeout, drop_probability)
    #[arg(long)]
    param: String,
    /// Start value (inclusive)
    #[arg(long)]
    start: f64,
    /// End value (inclusive)
    #[arg(long)]
    end: f64,
    /// Number of evenly-spaced steps
    #[arg(long, default_value = "10")]
    steps: usize,
    /// Override the RNG seed for every step (default: use seed from config)
    #[arg(long)]
    seed: Option<u64>,
    /// Output directory for the sweep CSV
    #[arg(short, long)]
    output: Option<PathBuf>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => run_single(args),
        Command::RunAll(args) => run_all(args),
        Command::SweepSeeds(args) => sweep_seeds(args),
        Command::Sweep(args) => sweep(args),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load a scenario config or print an error and exit with code 1.
fn load_config_or_exit(path: &Path) -> ScenarioConfig {
    match faultsim::scenario::load_config(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config {}: {}", path.display(), e);
            std::process::exit(1);
        }
    }
}

/// Create an output directory (including parents), exiting on failure.
fn make_output_dir(dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("Error creating output directory {}: {}", dir.display(), e);
        std::process::exit(1);
    }
}

/// Override one named parameter in the config with a new value.
/// All detector and network parameters that make sense to sweep are supported.
fn apply_param(config: &mut ScenarioConfig, param: &str, value: f64) -> Result<(), String> {
    match param {
        // Detector params
        "timeout" => config.detector.timeout = Some(value as u64),
        "alpha" => config.detector.alpha = Some(value),
        "safety_multiplier" => config.detector.safety_multiplier = Some(value),
        "suspicion_threshold" => config.detector.suspicion_threshold = Some(value as u32),
        "gossip_interval" => config.detector.gossip_interval = Some(value as u64),
        "gossip_fanout" => config.detector.gossip_fanout = Some(value as u32),
        "phi_threshold" => config.detector.phi_threshold = Some(value),
        "phi_window_size" => config.detector.phi_window_size = Some(value as usize),
        "phi_min_stddev" => config.detector.phi_min_stddev = Some(value),
        // Network params
        "drop_probability" => config.network.drop_probability = value,
        "base_latency" => config.network.base_latency = value as u64,
        "jitter" => config.network.jitter = value as u64,
        // Simulation params
        "max_ticks" => config.simulation.max_ticks = value as u64,
        "node_count" => config.cluster.node_count = value as u32,
        "heartbeat_interval" => config.cluster.heartbeat_interval = value as u64,
        // Any unrecognised key is treated as a custom detector parameter.
        // This makes every entry in [detector.params] sweepable automatically.
        _ => {
            config.detector.params.insert(param.to_string(), value);
        }
    }
    Ok(())
}

/// Write all result files for a single run to `dir`.
/// Format is `"csv"` or `"json"` (anything else falls back to CSV).
/// The φ log is always written as CSV regardless of `format`.
fn export_results(
    metrics: &faultsim::metrics::MetricsCollector,
    dir: &Path,
    scenario_name: &str,
    max_ticks: u64,
    format: &str,
    wall_time_ms: f64,
) {
    match format {
        "json" => {
            if let Err(e) = metrics.export_detections_json(&dir.join("detections.json")) {
                eprintln!("Error writing detections JSON: {}", e);
            }
            if let Err(e) = metrics.export_summary_json(
                &dir.join("summary.json"),
                max_ticks,
                scenario_name,
                wall_time_ms,
            ) {
                eprintln!("Error writing summary JSON: {}", e);
            }
        }
        _ => {
            // Default: CSV
            if let Err(e) = metrics.export_detections_csv(&dir.join("detections.csv")) {
                eprintln!("Error writing detections CSV: {}", e);
            }
            if let Err(e) = metrics.export_summary_csv(
                &dir.join("summary.csv"),
                max_ticks,
                scenario_name,
                wall_time_ms,
            ) {
                eprintln!("Error writing summary CSV: {}", e);
            }
        }
    }
    // φ log is always CSV (time-series tabular data); format flag does not affect it.
    if !metrics.phi_log.is_empty() {
        let phi_path = dir.join(format!("{}_phi_log.csv", scenario_name));
        if let Err(e) = metrics.export_phi_log_csv(&phi_path) {
            eprintln!("Error writing phi log CSV: {}", e);
        } else {
            println!("φ log:           {}", phi_path.display());
        }
        let events_path = dir.join(format!("{}_events.csv", scenario_name));
        if let Err(e) = metrics.export_events_csv(&events_path) {
            eprintln!("Error writing events CSV: {}", e);
        } else {
            println!("Events:          {}", events_path.display());
        }
    }
    println!("Results exported to {}", dir.display());
}

// ── `run` ─────────────────────────────────────────────────────────────────────

fn run_single(args: RunArgs) {
    let config = load_config_or_exit(&args.config);
    let mut engine = faultsim::scenario::build_engine(&config, args.seed);
    engine.run();

    faultsim::scenario::print_summary(&engine.metrics, config.simulation.max_ticks);
    println!("  Wall-clock time:  {:.2}ms", engine.run_time_ms);

    let output_dir = args.output.or_else(|| {
        config
            .output
            .as_ref()
            .and_then(|o| o.dir.as_ref())
            .map(PathBuf::from)
    });

    if let Some(dir) = output_dir {
        make_output_dir(&dir);
        let scenario_name = args
            .config
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let format = config
            .output
            .as_ref()
            .and_then(|o| o.format.as_deref())
            .unwrap_or("csv");
        export_results(
            &engine.metrics,
            &dir,
            scenario_name,
            config.simulation.max_ticks,
            format,
            engine.run_time_ms,
        );
    }
}

// ── `run-all` ─────────────────────────────────────────────────────────────────

fn run_all(args: RunAllArgs) {
    let entries = match std::fs::read_dir(&args.scenarios) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading scenarios directory: {}", e);
            std::process::exit(1);
        }
    };

    let mut toml_paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("toml"))
        .collect();
    toml_paths.sort();

    if toml_paths.is_empty() {
        eprintln!("No .toml files found in {:?}", args.scenarios);
        std::process::exit(1);
    }

    make_output_dir(&args.output);

    let summary_path = args.output.join("summary.csv");
    let _ = std::fs::remove_file(&summary_path);

    println!("Running {} scenarios...\n", toml_paths.len());

    let batch_start = Instant::now();

    for path in &toml_paths {
        let scenario_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let config = match faultsim::scenario::load_config(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  [SKIP] {}: {}", scenario_name, e);
                continue;
            }
        };

        let mut engine = faultsim::scenario::build_engine(&config, None);
        engine.run();

        let detections_path = args
            .output
            .join(format!("{}_detections.csv", scenario_name));
        if let Err(e) = engine.metrics.export_detections_csv(&detections_path) {
            eprintln!(
                "  [WARN] {}: failed to write detections CSV: {}",
                scenario_name, e
            );
        }

        if let Err(e) = engine.metrics.export_summary_csv(
            &summary_path,
            config.simulation.max_ticks,
            scenario_name,
            engine.run_time_ms,
        ) {
            eprintln!(
                "  [WARN] {}: failed to write summary row: {}",
                scenario_name, e
            );
        }

        println!(
            "  {:32}  FP={:.4}  FN={:2}  mean_lat={}  time={:.2}ms",
            scenario_name,
            engine.metrics.false_positive_rate(),
            engine.metrics.false_negative_count(),
            engine
                .metrics
                .mean_detection_latency()
                .map_or("N/A".to_string(), |l| format!("{:.1}", l)),
            engine.run_time_ms,
        );
    }

    let batch_ms = batch_start.elapsed().as_millis();
    println!(
        "\nDone. {} scenarios in {}ms. Summary written to {}",
        toml_paths.len(),
        batch_ms,
        summary_path.display()
    );
}

// ── `sweep-seeds` ─────────────────────────────────────────────────────────────

fn sweep_seeds(args: SweepSeedsArgs) {
    let config = load_config_or_exit(&args.config);
    let scenario_name = args
        .config
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    println!(
        "sweep-seeds: {} seeds × scenario '{}'\n",
        args.seeds, scenario_name
    );

    let mut snapshots: Vec<RunSnapshot> = Vec::with_capacity(args.seeds as usize);

    for seed in 1..=args.seeds {
        let mut engine = faultsim::scenario::build_engine(&config, Some(seed));
        engine.run();
        snapshots.push(RunSnapshot::from_metrics(
            &engine.metrics,
            seed,
            engine.run_time_ms,
        ));
        eprint!(
            "  seed {:>4}  FP={:.4}  FN={}  time={:.2}ms\r",
            seed,
            snapshots.last().unwrap().false_positive_rate,
            snapshots.last().unwrap().false_negative_count,
            engine.run_time_ms
        );
    }
    eprintln!(); // clear the \r line

    let agg = AggregatedMetrics::from_snapshots(&snapshots);
    println!();
    agg.print();

    if let Some(dir) = &args.output {
        make_output_dir(dir);

        let runs_path = dir.join(format!("{}_runs.csv", scenario_name));
        if let Err(e) = export_runs_csv(&snapshots, &runs_path) {
            eprintln!("Error writing runs CSV: {}", e);
        } else {
            println!("\nPer-run data:    {}", runs_path.display());
        }

        let agg_path = dir.join(format!("{}_agg.csv", scenario_name));
        if let Err(e) = agg.export_csv(&agg_path) {
            eprintln!("Error writing aggregated CSV: {}", e);
        } else {
            println!("Aggregated data: {}", agg_path.display());
        }
    }
}

// ── `sweep` ───────────────────────────────────────────────────────────────────

fn sweep(args: SweepArgs) {
    if args.steps == 0 {
        eprintln!("--steps must be at least 1");
        std::process::exit(1);
    }

    let base_config = load_config_or_exit(&args.config);
    let scenario_name = args
        .config
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Generate evenly-spaced values (inclusive on both ends).
    let step_size = if args.steps == 1 {
        0.0
    } else {
        (args.end - args.start) / (args.steps - 1) as f64
    };
    let values: Vec<f64> = (0..args.steps)
        .map(|i| args.start + i as f64 * step_size)
        .collect();

    println!(
        "sweep: {} from {:.4} → {:.4} ({} steps), scenario '{}'\n",
        args.param, args.start, args.end, args.steps, scenario_name
    );

    // Print header.
    println!(
        "{:>14}  {:>10}  {:>6}  {:>12}  {:>10}  {:>10}  {:>10}",
        args.param, "fp_rate", "fn", "mean_lat", "p50", "p95", "p99"
    );
    println!("{}", "-".repeat(78));

    let mut rows: Vec<(f64, RunSnapshot)> = Vec::with_capacity(args.steps);

    for value in &values {
        let mut config = base_config.clone();
        if let Err(e) = apply_param(&mut config, &args.param, *value) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }

        let seed = args.seed.unwrap_or(config.simulation.seed);
        let mut engine = faultsim::scenario::build_engine(&config, Some(seed));
        engine.run();

        let snap = RunSnapshot::from_metrics(&engine.metrics, seed, engine.run_time_ms);
        let fmt_lat = |v: f64| {
            if v.is_nan() {
                "N/A".to_string()
            } else {
                format!("{:.2}", v)
            }
        };

        println!(
            "{:>14.4}  {:>10.4}  {:>6}  {:>12}  {:>10}  {:>10}  {:>10}",
            value,
            snap.false_positive_rate,
            snap.false_negative_count,
            fmt_lat(snap.mean_detection_latency),
            fmt_lat(snap.p50_latency),
            fmt_lat(snap.p95_latency),
            fmt_lat(snap.p99_latency),
        );

        rows.push((*value, snap));
    }

    if let Some(dir) = &args.output {
        make_output_dir(dir);
        let csv_name = format!("{}_{}.csv", scenario_name, args.param);
        let csv_path = dir.join(&csv_name);
        if let Err(e) = export_sweep_csv(&rows, &args.param, &csv_path) {
            eprintln!("Error writing sweep CSV: {}", e);
        } else {
            println!("\nSweep data written to {}", csv_path.display());
        }
    }
}
