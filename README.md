# faultsim

**Failure Misclassification in Distributed Clusters: A Simulation Study Under Jitter, Churn, and Partitions**

`faultsim` is a discrete-event simulator for studying how failure-detection strategies behave under unstable network conditions. It focuses on *misclassification* — when healthy nodes are incorrectly declared failed due to jitter, delay, churn, or network partitions.

## Research Question

> Under what network conditions do common failure-detection strategies misclassify healthy nodes as failed, and how do adaptive or gossip-assisted approaches compare to fixed-timeout methods?

## Strategies Under Study

| Strategy | Description |
|---|---|
| **Fixed-timeout heartbeat** | Declares failure after a static timeout with no heartbeat received |
| **Adaptive-timeout heartbeat** | Adjusts the timeout dynamically based on observed round-trip behavior |
| **Gossip-assisted suspicion** | Combines local heartbeat monitoring with disseminated suspicion via gossip |

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
cargo build

# Run with a scenario config
cargo run -- --config configs/scenarios/baseline.toml

# Run tests
cargo test

# Check formatting
cargo fmt -- --check
```

## Development

This project uses a `main`/`dev` branch model with feature branches:

- `main` — stable, passing CI, tagged releases
- `dev` — integration branch for in-progress work
- `feature/*` — individual feature branches merged into `dev`

See [docs/design.md](docs/design.md) for architecture details and [docs/experiment-plan.md](docs/experiment-plan.md) for the experimental methodology.

## License

[MIT](LICENSE)
