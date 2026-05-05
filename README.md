# faultsim

**Failure Misclassification in Distributed Clusters: A Simulation Study Under Jitter, Churn, and Partitions**

`faultsim` is a discrete-event simulator for studying how failure-detection strategies behave under unstable network conditions. It focuses on misclassification: healthy nodes being declared failed because of jitter, delay, churn, packet loss, or partitions.

## Research Question

> Under what network conditions do common failure-detection strategies misclassify healthy nodes as failed, and how do adaptive or gossip-assisted approaches compare to fixed-timeout methods?

## Strategies Under Study

| Strategy | Description |
|---|---|
| **Fixed-timeout heartbeat** | Declares failure after a static timeout with no heartbeat received |
| **Adaptive-timeout heartbeat** | EWMA of observed inter-arrival times drives the timeout |
| **Gossip-assisted suspicion** | Combines local heartbeat monitoring with suspicion disseminated via gossip |
| **Phi accrual** | Suspicion level derived from a distribution over recent inter-arrival samples |
| **Adaptive accrual** | Accrual-style detector with parameters tuned online |
| **Custom** | User-defined detector implemented in `src/detector/custom.rs` |

## Key Metrics

- **False positive rate** - fraction of healthy nodes incorrectly declared failed
- **False negatives** - crashed nodes that were never detected
- **Detection latency** - time from actual failure to detection, including p50/p95/p99 summaries
- **Message overhead** - total messages delivered during the run
- **Wall-clock runtime** - simulator execution time in milliseconds (`wall_time_ms` in CSV/JSON outputs)

## Project Structure

```text
src/            Simulator source code (Rust)
docs/           Design notes, experiment plan, demo guide, custom-detector guide
configs/        Scenario configuration files (TOML)
scripts/        Plotting and experiment helper scripts
results/        Generated output (git-ignored, except .gitkeep)
tests/          Integration and smoke tests
```

## Getting Started

```bash
# Build the simulator
cargo build
cargo build --release

# Run one scenario
cargo run --release -- run --config configs/scenarios/baseline.toml
cargo run --release -- run --config configs/scenarios/baseline.toml --seed 42 --output results/demo

# Run every scenario in a directory and write a combined summary.csv
cargo run --release -- run-all --scenarios configs/scenarios --output results/batch

# Re-run one scenario across many seeds for aggregate statistics
cargo run --release -- sweep-seeds --config configs/scenarios/baseline.toml --seeds 30 --output results/seeds

# Sweep one detector or network parameter over a range
cargo run --release -- sweep --config configs/scenarios/crash_phi_accrual.toml \
    --param phi_threshold --start 1.0 --end 16.0 --steps 10 --output results/phi_sweep

# Test and lint
cargo test
cargo fmt -- --check
cargo clippy -- -D warnings

# Plot a combined summary.csv into publication-style figures
python scripts/plot_results.py results/batch/summary.csv --output results/plots
```

The plotting script expects `matplotlib` and `pandas`. For a batch summary it generates `fp_rate.png`, `latency.png`, `strategy_comparison.png`, and `wall_time.png`. For sweep CSVs it generates `sweep_<param>.png`.

Single-scenario runs can export CSV or JSON summaries plus detection logs. `run-all` always writes a combined `summary.csv`, and the aggregate CSV outputs now include `wall_time_ms`.

See [docs/demo.md](docs/demo.md) for a guided walkthrough of the main workflows.

## Scenarios

[configs/scenarios/](configs/scenarios/) holds the full scenario set. Each TOML specifies cluster size, network parameters, detector strategy, and a fault schedule (`crash`, `recover`, `partition_start`, `partition_end`).

Current scenario groups include:

- **baseline / adaptive / gossip** - clean-network baselines
- **crash_*** - single-node crash scenarios for each detector
- **high_jitter_*** - elevated jitter stress tests
- **drops_5pct / drops_15pct** - lossy-link scenarios
- **partition** - partition and heal scenarios
- **custom_example** - starter scenario for user-defined detectors

See [configs/scenarios/README.md](configs/scenarios/README.md) for the full config schema and scenario conventions.

## Custom Detectors

To add your own detector:

1. Edit [src/detector/custom.rs](src/detector/custom.rs).
2. Set `strategy = "custom"` in a scenario TOML.
3. Put any detector-specific knobs under `[detector.params]`.
4. Run the scenario normally, or sweep those custom parameters with `faultsim sweep`.

The engine, metrics export, seed sweeps, parameter sweeps, and plotting workflow all work with the custom strategy. See [docs/custom-detector.md](docs/custom-detector.md) for the full guide and worked example.

## Reproducibility

Every run is deterministic for a given `(config, seed)` pair. A single `StdRng` is threaded through [src/scenario.rs](src/scenario.rs) into the engine and network model, and the `deterministic_replay` integration test guards against accidental nondeterminism.

## Development

Use short-lived branches for focused changes and merge them back into `main` once tests are passing. Keep noteworthy design or workflow decisions documented in [docs/logs/](docs/logs/).

For more context, see [docs/design.md](docs/design.md), [docs/experiment-plan.md](docs/experiment-plan.md), and [docs/custom-detector.md](docs/custom-detector.md).
