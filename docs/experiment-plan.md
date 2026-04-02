# Experiment Plan

## Goal

Systematically measure how three failure-detection strategies perform across a range of adverse network conditions, focusing on false-positive rates and detection latency.

## Independent Variables

### Network Conditions

| Variable | Values |
|---|---|
| Base latency | 10ms, 50ms, 200ms (simulated ticks) |
| Jitter magnitude | Low (5%), Medium (25%), High (75% of base) |
| Jitter distribution | Uniform, Normal, Heavy-tailed (Pareto) |
| Packet drop rate | 0%, 1%, 5%, 15% |
| Partition type | None, Symmetric, Asymmetric |
| Partition duration | Transient (100 ticks), Sustained (1000 ticks) |

### Cluster Parameters

| Variable | Values |
|---|---|
| Cluster size | 5, 10, 25, 50 nodes |
| Churn rate | 0, 1, 5 joins/leaves per 1000 ticks |
| Crash count | 0, 1, 3 nodes (injected at scheduled ticks) |

### Detection Strategies

1. **Fixed-timeout** — timeout values: 100, 200, 500 ticks
2. **Adaptive-timeout** — EWMA alpha: 0.1, 0.3; safety multiplier: 2x, 4x
3. **Gossip-assisted** — suspicion threshold: 2, 3 confirmations; gossip interval: 50, 100 ticks

## Dependent Variables (Metrics)

- **False positive rate** — `false_positives / total_detection_events`
- **Detection latency** — ticks between actual failure and detection
- **Recovery time** — ticks between fault resolution and correct cluster view
- **Messages per tick** — average message count across the run

## Experiment Phases

### Phase 1: Baseline Characterization

Run each strategy under clean network conditions (no jitter, no drops, no partitions) across cluster sizes. Establish baseline detection latency and confirm zero false positives.

### Phase 2: Jitter Sensitivity

Fix cluster size at 10. Sweep jitter magnitude and distribution for each strategy. Measure false-positive rate and detection latency as jitter increases.

### Phase 3: Partition Behavior

Fix cluster size at 10. Introduce symmetric and asymmetric partitions of varying duration. Measure false-positive rate and recovery time.

### Phase 4: Churn Stress

Fix cluster size at 25. Vary churn rates. Measure detection accuracy and messaging overhead.

### Phase 5: Combined Adversarial

Select the two most stressful parameter combinations from phases 2–4. Run all strategies under these combined conditions across cluster sizes 10, 25, 50.

## Reproducibility

- Every scenario is defined by a TOML config file checked into `configs/scenarios/`
- Each config specifies an RNG seed for deterministic replay
- Results are written to `results/` with the config filename and timestamp
- Analysis scripts in `scripts/` read results and produce plots

## Analysis Plan

- Tabular summaries per strategy per condition
- Line plots: false-positive rate vs. jitter magnitude, per strategy
- Heatmaps: detection latency across (latency, jitter) grid
- Bar charts: messaging overhead comparison
- Statistical tests where sample sizes permit (multiple seeds per config)
