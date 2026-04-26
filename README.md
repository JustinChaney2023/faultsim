# faultsim

**Failure Misclassification in Distributed Clusters: A Simulation Study Under Jitter, Churn, and Partitions**

`faultsim` is a discrete-event simulator for studying how failure-detection strategies behave under unstable network conditions. It focuses on *misclassification* — when healthy nodes are incorrectly declared failed due to jitter, delay, churn, or network partitions.

## Research Question

> Under what network conditions do common failure-detection strategies misclassify healthy nodes as failed, and how do adaptive or gossip-assisted approaches compare to fixed-timeout methods?

## Strategies Under Study

| Strategy | Description |
|---|---|
| **Fixed-timeout heartbeat** | Declares failure after a static timeout with no heartbeat received |
| **Adaptive-timeout heartbeat** | EWMA of observed inter-arrival times drives the timeout |
| **Gossip-assisted suspicion** | Combines local heartbeat monitoring with suspicion disseminated via gossip |
| **Phi accrual** | Suspicion level derived from a distribution over recent inter-arrival samples (Hayashibara et al.) |
| **Adaptive accrual** | Accrual-style detector with parameters tuned online |

## Key Metrics

- **Detection latency** — time from actual failure to detection
- **False positive rate** — fraction of healthy nodes incorrectly declared failed
- **Recovery/convergence time** — time for the cluster to reach a correct view after a transient fault
- **Messaging overhead** — total messages exchanged per detection cycle

## Project Structure

```
src/            Simulator source code (Rust)
docs/           Research proposal, design document, experiment plan
configs/        Scenario configuration files (TOML)
scripts/        Experiment runners and plotting helpers
results/        Generated output (git-ignored, except .gitkeep)
tests/          Integration and smoke tests
```

## Getting Started

```bash
# Build the simulator
cargo build                  # debug
cargo build --release        # optimized, used by the batch runner

# Run with a scenario config
cargo run --release -- --config configs/scenarios/baseline.toml

# Run with a fixed seed and output directory
cargo run --release -- --config configs/scenarios/baseline.toml --seed 42 --output results/demo

# Run tests (27 unit + 9 integration)
cargo test

# CI gates
cargo fmt -- --check
cargo clippy -- -D warnings

# Batch a set of scenarios; results land in results/<scenario>_<timestamp>/
./scripts/run_experiment.sh configs/scenarios/*.toml

# Plot aggregated summaries
python scripts/plot_results.py results/**/summary.csv -o results/plots
```

See [docs/demo.md](docs/demo.md) for a guided set of demo commands covering every detector and network-pathology scenario.

## Scenarios

[configs/scenarios/](configs/scenarios/) holds the full scenario set. Each TOML specifies cluster size, network parameters, detector strategy, and a fault schedule (`crash`, `recover`, `partition_start`, `partition_end`). Groups currently covered:

- **baseline / adaptive / gossip** — clean-network baselines per detector
- **crash_\*** — single-node crash under each detector strategy
- **high_jitter_\*** — elevated jitter stress tests
- **drops_5pct / drops_15pct** — lossy-link scenarios
- **partition** — network partition and heal

## Reproducibility

Every run is deterministic given a `(config, seed)` pair. A single `StdRng` is threaded through [src/scenario.rs](src/scenario.rs) into the engine and network model; no uncontrolled randomness (`thread_rng`, unordered `HashMap` iteration on observable paths) is introduced. The `deterministic_replay` integration test enforces this.

## Development

This project uses a `main`/`dev` branch model with feature branches:

- `main` — stable, passing CI, tagged releases
- `dev` — integration branch for in-progress work
- `feature/*` — individual feature branches merged into `dev`

Every notable change is accompanied by a dated entry in [docs/logs/](docs/logs/) capturing the *why*, not just the *what*.

See [docs/design.md](docs/design.md) for architecture details and [docs/experiment-plan.md](docs/experiment-plan.md) for the experimental methodology.
