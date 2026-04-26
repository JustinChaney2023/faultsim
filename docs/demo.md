# Demo — Test & Run Commands

A walkthrough of commands for demonstrating `faultsim` in a live session. Each section is self-contained; you can skip any block. All commands are run from the repo root.

## 1. Build

```bash
cargo build                  # debug build, fastest to compile
cargo build --release        # optimized binary used by the batch runner
```

## 2. Quality checks (CI gates)

```bash
cargo fmt -- --check         # formatting
cargo clippy -- -D warnings  # lints, warnings treated as errors
```

## 3. Tests

```bash
cargo test                                   # all 27 unit + 9 integration tests
cargo test --test smoke                      # integration tests only
cargo test deterministic_replay              # single test by name
cargo test -- --nocapture                    # show println! output
```

Representative integration tests (in [tests/smoke.rs](../tests/smoke.rs)):

| Test | What it demonstrates |
|---|---|
| `baseline_simulation_no_false_positives` | Clean network, no faults → zero misclassifications |
| `crash_detected_as_true_positive` | A real crash produces exactly one detection |
| `deterministic_replay` | Same config + seed → bit-for-bit identical run |
| `engine_runs_to_completion` | Event loop halts on `max_ticks` |
| `network_delivers_within_bounds` | Latency + jitter model stays within configured bounds |

## 4. Single-scenario runs

Each `.toml` in [configs/scenarios/](../configs/scenarios/) is a self-contained experiment (cluster size, network params, detector, fault schedule).

```bash
# Clean baseline, fixed-timeout detector
cargo run --release -- --config configs/scenarios/baseline.toml

# Same scenario, explicit seed and output directory
cargo run --release -- --config configs/scenarios/baseline.toml --seed 123 --output results/demo_baseline
```

### Comparing the three detector strategies on a crash

```bash
cargo run --release -- --config configs/scenarios/crash_recovery.toml        # fixed-timeout
cargo run --release -- --config configs/scenarios/crash_adaptive.toml        # adaptive EWMA
cargo run --release -- --config configs/scenarios/crash_gossip.toml          # gossip-assisted
cargo run --release -- --config configs/scenarios/crash_phi_accrual.toml     # phi accrual
cargo run --release -- --config configs/scenarios/crash_adaptive_accrual.toml
```

### Stressing the detectors with network pathologies

```bash
cargo run --release -- --config configs/scenarios/high_jitter.toml                 # fixed timeout under jitter
cargo run --release -- --config configs/scenarios/high_jitter_adaptive.toml        # adaptive under jitter
cargo run --release -- --config configs/scenarios/high_jitter_phi_accrual.toml
cargo run --release -- --config configs/scenarios/drops_5pct.toml                  # 5% drop rate
cargo run --release -- --config configs/scenarios/drops_15pct.toml                 # 15% drop rate
cargo run --release -- --config configs/scenarios/partition.toml                   # network partition
```

## 5. Batch experiments

Run every scenario in one pass. Results land in `results/<scenario>_<timestamp>/` with the config copied alongside for reproducibility.

```bash
./scripts/run_experiment.sh configs/scenarios/*.toml
```

Run a focused subset (e.g. the detector comparison used in [docs/results/2026-04-15_detector-comparison.md](results/2026-04-15_detector-comparison.md)):

```bash
./scripts/run_experiment.sh \
    configs/scenarios/crash_recovery.toml \
    configs/scenarios/crash_adaptive.toml \
    configs/scenarios/crash_gossip.toml \
    configs/scenarios/crash_phi_accrual.toml \
    configs/scenarios/crash_adaptive_accrual.toml
```

## 6. Plots

```bash
python scripts/plot_results.py results/**/summary.csv -o results/plots
```

## 7. Output anatomy

Each run produces, in its output directory:

- `detections.csv` — one row per detection event: `(tick, detector, suspected_node, true_positive, latency_from_crash)`
- `summary.csv` — per-run aggregate: detection counts, false-positive rate, mean detection latency, message overhead
- `config.toml` — copy of the input scenario (batch runner only)
- `output.txt` — stdout/stderr (batch runner only)

## 8. Reproducibility

Every run is deterministic given a `(config, seed)` pair. The single `StdRng` is threaded through [scenario::build_engine](../src/scenario.rs) into the network model; no `thread_rng()` or unordered iteration is used on observable paths. To demonstrate:

```bash
cargo run --release -- --config configs/scenarios/crash_recovery.toml --seed 7 --output results/rep_a
cargo run --release -- --config configs/scenarios/crash_recovery.toml --seed 7 --output results/rep_b
diff results/rep_a/detections.csv results/rep_b/detections.csv    # empty
```

## 9. Further reading

- [docs/proposal.md](proposal.md) — research question, motivation
- [docs/design.md](design.md) — architecture of the event loop and detector trait
- [docs/experiment-plan.md](experiment-plan.md) — experimental methodology
- [docs/results/2026-04-15_detector-comparison.md](results/2026-04-15_detector-comparison.md) — current results writeup
- [CLAUDE.md](../CLAUDE.md) — developer-facing architecture notes
